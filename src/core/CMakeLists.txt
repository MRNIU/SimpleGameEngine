
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# CMakeLists.txt for Simple-XX/SimpleGameEngine.

add_library(core
        log.cpp
)

target_include_directories(core PRIVATE
        include
)

target_compile_options(core PRIVATE
        ${DEFAULT_COMPILE_OPTIONS}
)

target_link_libraries(core PRIVATE
        ${DEFAULT_LINK_LIB}
)
