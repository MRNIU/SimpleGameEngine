
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

#ifndef SIMPLEGAMEENGINE_SRC_CORE_CONFIG_CONFIG_H
#define SIMPLEGAMEENGINE_SRC_CORE_CONFIG_CONFIG_H

#include <cstdint>
#include <string>

namespace simple_game_engine {
namespace core {

/**
 * 配置信息
 */
class Config {
 public:
  /**
   * 构造函数
   * @param config_json_file_path 配置文件路径
   */
  Config(const std::string &config_json_file_path);

  /// @name 默认构造/析构函数
  /// @{
  Config() = delete;
  Config(const Config &_scene) = delete;
  Config(Config &&_scene) = delete;
  auto operator=(const Config &_scene) -> Config & = delete;
  auto operator=(Config &&_scene) -> Config & = delete;
  ~Config() = default;
  /// @}

  /**
   * 获取 app 名称
   * @return app 名称
   */
  const std::string &GetApplicationName() const;

  /**
   * 获取屏幕宽度
   * @return 屏幕宽度
   */
  size_t GetScreenWidth() const;
  /**
   * 获取屏幕高度
   * @return 屏幕高度
   */
  size_t GetScreenHeight() const;

  /**
   * 获取日志文件路径
   * @return 日志文件路径
   */
  const std::string &GetLogFilePath() const;
  /**
   * 获取日志文件最大大小
   * @return 日志文件最大大小
   */
  size_t GetLogFileMaxSize() const;
  /**
   * 获取日志文件最大数量
   * @return 日志文件最大数量
   */
  size_t GetLogFileMaxCount() const;

 private:
  /// app 名称
  std::string application_name_;

  /// 屏幕宽度
  size_t screen_width_;
  /// 屏幕高度
  size_t screen_height_;

  /// 日志文件路径
  std::string log_file_path_;
  /// 日志文件最大大小
  size_t log_file_max_size_;
  /// 日志文件最大数量
  size_t log_file_max_count_;
};

}  // namespace core
}  // namespace simple_game_engine

#endif  // SIMPLEGAMEENGINE_SRC_CORE_CONFIG_CONFIG_H
