
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# CMakeLists.txt for Simple-XX/SimpleGameEngine.

add_library(core
        application/application.cpp
        config/config.cpp
        log/log_system.cpp
)

target_include_directories(core PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
)

target_compile_options(core PRIVATE
        ${DEFAULT_COMPILE_OPTIONS}
)

target_link_libraries(core PRIVATE
        ${DEFAULT_LINK_LIB}
)
