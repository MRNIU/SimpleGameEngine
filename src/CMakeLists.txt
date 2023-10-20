
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# CMakeLists.txt for Simple-XX/SimpleGameEngine.

include_directories("Framework/Codec")
include_directories("Framework/Common")
include_directories("Framework/Interface")
include_directories("Framework/GeomMath")
include_directories("RHI")
IF(${UNIX})
    include_directories("Platform/Linux")
ELSEIF(${WIN32})
    include_directories("Platform/Windows")
ENDIF(${UNIX})

include(CTest)
add_subdirectory(Framework)
add_subdirectory(Platform)
add_subdirectory(RHI)

# 生成可执行文件
# add_executable(SimpleGameEngine
#         main.cpp
#         log.cpp
# )

# target_include_directories(SimpleGameEngine PRIVATE
#         include
# )

# target_compile_options(SimpleGameEngine PRIVATE
#         ${DEFAULT_COMPILE_OPTIONS}
# )

# target_link_libraries(SimpleGameEngine PRIVATE
#         ${DEFAULT_LINK_LIB}
# )
