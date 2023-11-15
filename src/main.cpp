
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

#include "core/config/config.h"
#include "core/log/log_system.h"

// @todo 不应该出现明确的类型，应该使用模板
auto main(int, char **) -> int {
  simple_game_engine::core::Config config("config.json");
  simple_game_engine::core::LogSystem log_system(config.GetLogFilePath(),
                                                 config.GetLogFileMaxSize(),
                                                 config.GetLogFileMaxCount());

  log_system.info(233);

  return 0;
}
