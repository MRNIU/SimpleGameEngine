
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# CMakeLists.txt for Simple-XX/SimpleGameEngine.

# 生成可执行文件
add_executable(SimpleGameEngine
        main.cpp
        log.cpp
)

target_include_directories(SimpleGameEngine PRIVATE
        include
)

target_compile_options(SimpleGameEngine PRIVATE
        ${DEFAULT_COMPILE_OPTIONS}
)

target_link_libraries(SimpleGameEngine PRIVATE
        ${DEFAULT_LINK_LIB}
)
