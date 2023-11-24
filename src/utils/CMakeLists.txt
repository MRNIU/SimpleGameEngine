
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# CMakeLists.txt for Simple-XX/SimpleGameEngine.

add_library(utils
        config/config.cpp
        log/log_system.cpp
)

target_include_directories(utils INTERFACE
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}>
)

target_compile_options(utils PRIVATE
        ${DEFAULT_COMPILE_OPTIONS}
)

target_link_libraries(utils PRIVATE
        spdlog::spdlog
        Boost::headers
        Boost::json
)
