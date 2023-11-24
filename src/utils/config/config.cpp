
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
#include <boost/property_tree/json_parser.hpp>
#include <boost/property_tree/ptree.hpp>
#include <fstream>
#include <iostream>
#include <string>

namespace simple_game_engine {
namespace utils {

Config::Config(const std::filesystem::path& config_json_file_path) {
  // 打开文件
  std::ifstream json_file(config_json_file_path);
  if (!json_file) {
    throw std::runtime_error(
        "Error opening json_file: " + config_json_file_path.string() + ".\n");
  }
  // 读取文件内容到字符串
  std::string json_str((std::istreambuf_iterator<char>(json_file)),
                       std::istreambuf_iterator<char>());

  // 解析 json 文件
  auto json_value = boost::json::parse(json_str);

  // 保存解析结果
  application_name_ = json_value.at("application_name").as_string();
  screen_width_ = json_value.at("screen_width").as_int64();
  screen_height_ = json_value.at("screen_height").as_int64();
  log_file_path_ = json_value.at("log_file_path").as_string();
  log_file_max_size_ = json_value.at("log_file_max_size").as_int64();
  log_file_max_count_ = json_value.at("log_file_max_count").as_int64();
}

const std::string& Config::GetApplicationName() const {
  return application_name_;
}

size_t Config::GetScreenWidth() const { return screen_width_; }

size_t Config::GetScreenHeight() const { return screen_height_; }

const std::string& Config::GetLogFilePath() const { return log_file_path_; }

size_t Config::GetLogFileMaxSize() const { return log_file_max_size_; }

size_t Config::GetLogFileMaxCount() const { return log_file_max_count_; }

}  // namespace utils
}  // namespace simple_game_engine
