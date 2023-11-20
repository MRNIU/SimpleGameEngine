
/**
 * @file main.cpp
 * @brief 入口
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-10-18
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleGameEngine
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-10-18<td>Zone.N<td>创建文件
 * </table>
 */

#include <string>
#include <string_view>

#include "core/config/config.h"
#include "core/log/log_system.h"
#include "platform/file_system/path.h"
#include "platform/vulkan/GlfwGeneral.hpp"

auto main(int, char**) -> int {
  auto config_file_path =
      simple_game_engine::platform::Path::GetExecutablePath()
          .parent_path()
          .append("config.json");
  simple_game_engine::core::Config config(config_file_path);
  simple_game_engine::core::LogSystem log_system(config.GetLogFilePath(),
                                                 config.GetLogFileMaxSize(),
                                                 config.GetLogFileMaxCount());

  SPDLOG_INFO("加载配置文件: {}", config_file_path.string());

  if (!InitializeWindow({280, 120})) return -1;  // 来个你讨厌的返回值
  while (!glfwWindowShouldClose(pWindow)) {
    TitleFps();
    /*渲染及操作过程，待填充*/
    glfwPollEvents();
  }
  TerminateWindow();

  return 0;
}
