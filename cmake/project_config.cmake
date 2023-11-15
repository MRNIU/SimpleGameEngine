
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# project_config.cmake for Simple-XX/SimpleGameEngine.
# 项目配置

# 应用程序名称
set(APPLICATION_NAME "${PROJECT_NAME}")
# 窗口宽度
set(SCREEN_WIDTH 1920)
# 窗口高度
set(SCREEN_HEIGHT 1080)
# 日志文件路径
set(LOG_FILE_PATH "${EXECUTABLE_OUTPUT_PATH}/logs/SimpleGameEngineLog.log")
# 日志文件大小 1024*1024*4, 4MB
set(LOG_FILE_MAX_SIZE 4194304)
# 日志文件数量
set(LOG_FILE_MAX_COUNT 8)

# 生成配置头文件
configure_file(
        "${PROJECT_SOURCE_DIR}/cmake/config.json.in"
        # "${PROJECT_SOURCE_DIR}/src/config.json"
        "${EXECUTABLE_OUTPUT_PATH}/config.json"
)
