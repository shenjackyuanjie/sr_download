\set ON_ERROR_STOP on
\set bad_hash '33d61d1a68021dde12f229a4add86e796c4dd5553a42cec57dd2b0f4a843e983'

\echo '=== sr_download database rebuild preflight ==='

SELECT current_database() AS database_name,
       current_schema() AS current_schema,
       current_setting('search_path') AS search_path;

DO $$
BEGIN
    IF to_regclass('public.main_data') IS NULL THEN
        RAISE EXCEPTION 'public.main_data does not exist';
    END IF;
    IF to_regclass('public.long_data') IS NULL THEN
        RAISE EXCEPTION 'public.long_data does not exist';
    END IF;
END
$$;

SELECT count(*) AS main_rows,
       count(*) FILTER (WHERE blake_hash = :'bad_hash') AS bad_hash_rows,
       count(*) FILTER (WHERE blake_hash <> :'bad_hash') AS clean_main_rows
FROM public.main_data;

SELECT count(*) AS long_rows,
       count(*) FILTER (WHERE md.save_id IS NULL) AS orphan_long_rows
FROM public.long_data ld
LEFT JOIN public.main_data md ON md.save_id = ld.save_id;

SELECT table_name
FROM information_schema.tables
WHERE table_schema = 'public'
  AND table_name = 'seaql_migrations';

SELECT version, updated_at
FROM public.db_version
ORDER BY updated_at DESC, version DESC;

\if :{?confirm_rebuild}
\else
\echo 'Preflight complete. No rebuild requested; rerun with -v confirm_rebuild=YES to continue.'
\quit
\endif

\if :confirm_rebuild
\else
\echo 'confirm_rebuild must be YES/true to execute the destructive rebuild.'
\quit
\endif

\if :{?reset_staging}
\if :reset_staging
\echo 'reset_staging enabled; removing previous staging tables'
DROP TABLE IF EXISTS public.long_data_clean;
DROP TABLE IF EXISTS public.main_data_clean;
\endif
\endif

DO $$
BEGIN
    IF to_regclass('public.main_data_clean') IS NOT NULL
       OR to_regclass('public.long_data_clean') IS NOT NULL THEN
        RAISE EXCEPTION 'staging table already exists; rerun with -v reset_staging=YES after reviewing it';
    END IF;
END
$$;

\echo '=== creating clean staging tables ==='

CREATE TABLE public.main_data_clean (
    save_id integer PRIMARY KEY,
    save_type public.save_type NOT NULL,
    blake_hash character(64) NOT NULL,
    len bigint NOT NULL,
    short_data character varying(1024),
    xml_tested boolean,
    time timestamp with time zone NOT NULL
);

CREATE TABLE public.long_data_clean (
    save_id integer PRIMARY KEY,
    len bigint NOT NULL,
    text character varying NOT NULL,
    CONSTRAINT save_id FOREIGN KEY (save_id)
        REFERENCES public.main_data_clean(save_id)
        ON UPDATE CASCADE
        ON DELETE CASCADE
);

INSERT INTO public.main_data_clean
    (save_id, save_type, blake_hash, len, short_data, xml_tested, time)
SELECT save_id, save_type, blake_hash, len, short_data, xml_tested, time
FROM public.main_data
WHERE blake_hash <> :'bad_hash';

INSERT INTO public.long_data_clean (save_id, len, text)
SELECT ld.save_id, ld.len, ld.text
FROM public.long_data ld
JOIN public.main_data_clean md ON md.save_id = ld.save_id;

CREATE INDEX maindata_savetype_saveid_idx_clean
    ON public.main_data_clean (save_type, save_id, len, xml_tested);

CREATE INDEX longdata_saveid_idx_clean
    ON public.long_data_clean (save_id);

CREATE INDEX idx_main_data_hash_covering_clean
    ON public.main_data_clean (blake_hash)
    INCLUDE (save_id, save_type, len);

ANALYZE public.main_data_clean;
ANALYZE public.long_data_clean;

\echo '=== validating clean staging tables ==='

SELECT count(*) AS clean_main_rows,
       count(*) FILTER (WHERE blake_hash = :'bad_hash') AS remaining_bad_hash_rows
FROM public.main_data_clean;

SELECT count(*) AS clean_long_rows,
       count(*) FILTER (WHERE md.save_id IS NULL) AS orphan_clean_long_rows
FROM public.long_data_clean ld
LEFT JOIN public.main_data_clean md ON md.save_id = ld.save_id;

SELECT count(*) AS missing_long_rows
FROM public.main_data_clean md
WHERE md.len > 1024
  AND NOT EXISTS (
      SELECT 1
      FROM public.long_data_clean ld
      WHERE ld.save_id = md.save_id
  );

DO $$
DECLARE
    old_main_rows bigint;
    old_bad_rows bigint;
    new_main_rows bigint;
    new_bad_rows bigint;
    expected_long_rows bigint;
    new_long_rows bigint;
    orphan_rows bigint;
    missing_long_rows bigint;
BEGIN
    SELECT count(*) INTO old_main_rows FROM public.main_data;
    SELECT count(*) INTO old_bad_rows
    FROM public.main_data
    WHERE blake_hash = '33d61d1a68021dde12f229a4add86e796c4dd5553a42cec57dd2b0f4a843e983';
    SELECT count(*) INTO new_main_rows FROM public.main_data_clean;
    SELECT count(*) INTO new_bad_rows
    FROM public.main_data_clean
    WHERE blake_hash = '33d61d1a68021dde12f229a4add86e796c4dd5553a42cec57dd2b0f4a843e983';
    SELECT count(*) INTO expected_long_rows
    FROM public.long_data ld
    JOIN public.main_data md ON md.save_id = ld.save_id
    WHERE md.blake_hash <> '33d61d1a68021dde12f229a4add86e796c4dd5553a42cec57dd2b0f4a843e983';
    SELECT count(*) INTO new_long_rows FROM public.long_data_clean;
    SELECT count(*) INTO orphan_rows
    FROM public.long_data_clean ld
    LEFT JOIN public.main_data_clean md ON md.save_id = ld.save_id
    WHERE md.save_id IS NULL;
    SELECT count(*) INTO missing_long_rows
    FROM public.main_data_clean md
    WHERE md.len > 1024
      AND NOT EXISTS (
          SELECT 1 FROM public.long_data_clean ld WHERE ld.save_id = md.save_id
      );

    IF new_bad_rows <> 0
       OR new_main_rows <> old_main_rows - old_bad_rows
       OR new_long_rows <> expected_long_rows
       OR orphan_rows <> 0
       OR missing_long_rows <> 0 THEN
        RAISE EXCEPTION
            'staging validation failed: old_main=%, bad=%, new_main=%, new_bad=%, expected_long=%, new_long=%, orphan=%, missing_long=%',
            old_main_rows, old_bad_rows, new_main_rows, new_bad_rows,
            expected_long_rows, new_long_rows, orphan_rows, missing_long_rows;
    END IF;
END
$$;

\echo '=== swapping tables; old data will be deleted ==='

BEGIN;
SET LOCAL lock_timeout = '5s';

DROP FUNCTION IF EXISTS public.update_xml_tested();
DROP VIEW IF EXISTS public.full_data;

DROP TABLE public.long_data;
DROP TABLE public.main_data;

ALTER TABLE public.main_data_clean RENAME TO main_data;
ALTER TABLE public.long_data_clean RENAME TO long_data;

ALTER INDEX public.main_data_clean_pkey RENAME TO main_data_pkey;
ALTER INDEX public.long_data_clean_pkey RENAME TO long_data_pkey;

ALTER INDEX public.maindata_savetype_saveid_idx_clean
    RENAME TO maindata_savetype_saveid_idx;
ALTER INDEX public.longdata_saveid_idx_clean
    RENAME TO longdata_saveid_idx;
ALTER INDEX public.idx_main_data_hash_covering_clean
    RENAME TO idx_main_data_hash_covering;

CREATE VIEW public.full_data AS
SELECT
    md.save_id,
    md.save_type,
    md.blake_hash,
    md.xml_tested,
    md.len,
    CASE
        WHEN md.len > 1024 THEN ld.text
        ELSE md.short_data
    END AS data
FROM public.main_data md
LEFT JOIN public.long_data ld ON md.save_id = ld.save_id;

CREATE FUNCTION public.update_xml_tested()
RETURNS VOID AS $$
BEGIN
    UPDATE public.main_data
    SET xml_tested = xml_is_well_formed_document(fd.data)
    FROM public.full_data fd
    WHERE public.main_data.save_id = fd.save_id
      AND public.main_data.xml_tested IS NULL
      AND fd.data IS NOT NULL;
END;
$$ LANGUAGE plpgsql;

DROP TABLE IF EXISTS public.seaql_migrations;

INSERT INTO public.db_version (version, updated_at)
VALUES (3, now())
ON CONFLICT (version) DO UPDATE SET updated_at = EXCLUDED.updated_at;

COMMIT;

ANALYZE public.main_data;
ANALYZE public.long_data;

\echo '=== rebuild complete ==='
SELECT count(*) AS remaining_bad_hash_rows
FROM public.main_data
WHERE blake_hash = :'bad_hash';
SELECT to_regclass('public.seaql_migrations') AS seaql_migrations_table;
SELECT count(*) AS full_data_rows FROM public.full_data;
