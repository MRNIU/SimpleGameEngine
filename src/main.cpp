
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
// #include "platform/vulkan/vk_engine.h"
#include <vk_mem_alloc.h>

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

  // VulkanEngine engine;
  // engine.init();
  // engine.run();
  // engine.cleanup();

  return 0;
}
