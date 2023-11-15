
/**
 * @file config.cpp
 * @brief 配置信息
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-11-15
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleGameEngine
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-11-15<td>Zone.N<td>创建文件
 * </table>
 */

#include "config.h"

#include <boost/json.hpp>

namespace simple_game_engine {
namespace core {

Config::Config(const std::string& config_json_file_path) {
  auto json_value = boost::json::parse(config_json_file_path);
  application_name_ = json_value.at("application_name").as_string();
  screen_width_ = json_value.at("screen_width").as_uint64();
  screen_height_ = json_value.at("screen_height").as_uint64();
}

const std::string& Config::GetApplicationName() const {
  return application_name_;
}

uint64_t Config::GetScreenWidth() const { return screen_width_; }

uint64_t Config::GetScreenHeight() const { return screen_height_; }

const std::string& Config::GetLogFilePath() const { return log_file_path_; }

uint64_t Config::GetLogFileMaxSize() const { return log_file_max_size_; }

uint64_t Config::GetLogFileMaxCount() const { return log_file_max_count_; }

}  // namespace core
}  // namespace simple_game_engine
