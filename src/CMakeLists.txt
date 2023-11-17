
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# CMakeLists.txt for Simple-XX/SimpleGameEngine.

add_subdirectory(platform)
add_subdirectory(core)
# add_subdirectory(function)

# 生成可执行文件
add_executable(SimpleGameEngine
        main.cpp
)

target_include_directories(SimpleGameEngine PRIVATE
)

target_compile_options(SimpleGameEngine PRIVATE
        ${DEFAULT_COMPILE_OPTIONS}
)

target_link_libraries(SimpleGameEngine PRIVATE
        ${DEFAULT_LINK_LIB}
        platform
        core
)
