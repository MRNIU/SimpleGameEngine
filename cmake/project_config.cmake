
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# project_config.cmake for Simple-XX/SimpleGameEngine.
# 项目配置

# obj 文件路径
set(OBJ_FILE_PATH "${PROJECT_SOURCE_DIR}/obj/")
# 字体文件路径
set(FONT_FILE_PATH "${wqy_font_SOURCE_DIR}/wqy-zenhei.ttc")
# 线程数
include(ProcessorCount)
ProcessorCount(NPROC)
# 日志文件路径
set(LOG_FILE_PATH "${EXECUTABLE_OUTPUT_PATH}/logs/SimpleGameEngineLog.log")
# 日志文件大小
set(LOG_FILE_MAX_SIZE 1024*1024*4)
# 日志文件数量
set(LOG_FILE_MAX_COUNT 8)

# 生成配置头文件
configure_file(
        "${PROJECT_SOURCE_DIR}/cmake/config.h.in"
        "${PROJECT_SOURCE_DIR}/src/core/include/config.h"
)
