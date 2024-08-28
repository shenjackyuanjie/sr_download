from __future__ import annotations

import psycopg2
import tomli

from lib_not_dr import loggers

with open("../../config.toml", "rb") as f:
    CONFIG = tomli.load(f)

logger = loggers.config.get_logger("xml_parse")


def get_db():
    connect = psycopg2.connect(
        CONFIG["db"]["url"]
    )
    return connect


def main():
    db = get_db()
    db_cur = db.cursor()

    xml_fetch = """
WITH limited_full_data AS (
    SELECT save_id, data
    FROM public.full_data
    WHERE "save_type" != 'none'
      AND xml_is_well_formed_document(full_data."data")
    LIMIT 20
)
SELECT limited_full_data.save_id, array_agg(x.part_type) AS part_types, array_agg(x.part_id) AS part_ids
FROM limited_full_data,
     XMLTABLE (
        '//Ship/Parts/Part'
        PASSING BY VALUE xmlparse(document limited_full_data."data")
        COLUMNS part_type text PATH '@partType',
                part_id text PATH '@id'
    ) AS x
GROUP BY limited_full_data.save_id;
    """

    db_cur.execute(xml_fetch)
    logger.info(db_cur.fetchall())
    ...

main()
