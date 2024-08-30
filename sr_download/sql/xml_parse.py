from __future__ import annotations

import time
import xml.etree.ElementTree as ET

import psycopg2
import tomli

from lib_not_dr import loggers

with open("../../config.toml", "rb") as f:
    CONFIG = tomli.load(f)

logger = loggers.config.get_logger("xml_parse")
# logger.global_level = 10
result_logger = loggers.config.get_logger("result")
file = loggers.outstream.FileCacheOutputStream(file_name="result.log")
result_logger.add_output(file)

def get_db():
    connect = psycopg2.connect(
        CONFIG["db"]["url"]
    )
    return connect

def parse_part_types(xml_file):
    tree = ET.parse(xml_file)
    root = tree.getroot()
    
    # 定义命名空间
    namespace = {'ns': 'http://shenjack.top:81/files/DR/xsd/partlist.xsd'}
    
    part_dict = {}
    
    # 遍历所有 PartType 元素
    for part in root.findall('ns:PartType', namespace):
        part_id = part.get('id')
        part_mass = float(part.get('mass')) # type: ignore
        part_dict[part_id] = part_mass * 500 # 缩放因子
    
    return part_dict

def fetch_data(db_cur, offset, limit):
    xml_fetch = f"""
WITH data AS (
    SELECT save_id as id, data
    FROM public.full_data
    WHERE "save_type" = 'ship'
      AND full_data.xml_tested
      AND save_id >= 1200000
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
SELECT id, 
       array_agg(part_type) AS part_types,
       array_agg(part_count) AS part_counts
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
    part_mass = parse_part_types("PartList.xml")
    logger.debug(part_mass)
    
    db = get_db()
    db_cur = db.cursor()
    offset = 51500
    limit = 500

    target_mass = 775150
    delta = 100

    start_time = time.time()

    while True:
        datas = fetch_data(db_cur, offset, limit)
        if not datas:
            break
        for data in datas:
            logger.debug(data)
            # (id, ['pod-1', 'parachute-1', 'lander-1', 'engine-1', 'fuselage-1', 'fueltank-0'], [1, 1, 2, 1, 1, 1])
            # 解析这玩意
            # ship_mass = sum(part_mass[part] * data[2][idx] for part, idx in enumerate(data[1]))
            ship_mass = sum(part_mass[part] * count if part in part_mass else 0 for part, count in zip(data[1], data[2]))
            logger.debug(f"{data[0]}: {ship_mass}")
            # 如果在误差范围内，则输出
            if target_mass - delta <= ship_mass <= target_mass + delta:
                result_logger.info(f"Ship ID: {data[0]}")
                result_logger.info(f"Mass: {ship_mass}")
                result_logger.info(f"Part Types: {data[1]}")
                result_logger.info(f"Part Counts: {data[2]}")
                result_logger.info("=" * 20)
                logger.warn(f"Ship ID: {data[0]}")
                logger.warn(f"Mass: {ship_mass}")
                logger.warn(f"Part Types: {data[1]}")
                logger.warn(f"Part Counts: {data[2]}")
                logger.warn("=" * 20)
        offset += limit
        # 效率
        delta_t = time.time() - start_time
        speed = limit / delta_t
        logger.info(f"Offset: {offset} Speed: {speed:.2f} ships/s({delta_t:.2f}s) - {data[0]}")
        start_time = time.time()

main()
