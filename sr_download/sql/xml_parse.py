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

def fetch_data(db_cur, offset, limit):
    # xml_fetch = f"""
    # WITH data AS (
    #     SELECT save_id as id, data
    #     FROM public.full_data
    #     WHERE "save_type" != 'none'
    #       AND xml_is_well_formed_document(full_data."data")
    #     LIMIT {limit} OFFSET {offset}
    # )
    # SELECT data.id, string_agg(parts.part_type, '|') AS part_types
    # FROM data,
    #      XMLTABLE (
    #         '//Ship/Parts/Part'
    #         PASSING BY VALUE xmlparse(document data."data")
    #         COLUMNS part_type text PATH '@partType',
    #                 part_id text PATH '@id'
    #     ) AS parts
    # GROUP BY data.id;
    # """
    xml_fetch = f"""
    WITH data AS (
        SELECT save_id as id, data
        FROM public.full_data
        WHERE "save_type" != 'none'
          AND xml_is_well_formed_document(full_data."data")
        LIMIT {limit} OFFSET {offset}
    ),
    parts_data AS (
        SELECT data.id, parts.part_type
        FROM data,
             XMLTABLE (
                '//Ship/Parts/Part'
                PASSING BY VALUE xmlparse(document data."data")
                COLUMNS part_type text PATH '@partType',
                        part_id text PATH '@id'
            ) AS parts
    )
    SELECT id, string_agg(part_type || ':' || part_count, '|') AS part_types
    FROM (
        SELECT id, part_type, COUNT(part_type) AS part_count
        FROM parts_data
        GROUP BY id, part_type
    ) AS counted_parts
    GROUP BY id;
    """
    db_cur.execute(xml_fetch)
    return db_cur.fetchall()

def main():
    db = get_db()
    db_cur = db.cursor()
    offset = 0
    limit = 100
    while True:
        datas = fetch_data(db_cur, offset, limit)
        if not datas:
            break
        for data in datas:
            logger.info(data)
        offset += limit

main()
