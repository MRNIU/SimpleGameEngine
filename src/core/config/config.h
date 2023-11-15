
/**
 * @file config.h
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

#include <string>

namespace simple_game_engine {
namespace core {

class Config {
 public:
  Config(const std::string& config_json_file_path);

  const std::string& GetApplicationName() const;

  uint64_t GetScreenWidth() const;
  uint64_t GetScreenHeight() const;

  const std::string& GetLogFilePath() const;
  uint64_t GetLogFileMaxSize() const;
  uint64_t GetLogFileMaxCount() const;

 private:
  std::string application_name_;

  uint64_t screen_width_;
  uint64_t screen_height_;

  std::string log_file_path_;
  uint64_t log_file_max_size_;
  uint64_t log_file_max_count_;
};

}  // namespace core
}  // namespace simple_game_engine
