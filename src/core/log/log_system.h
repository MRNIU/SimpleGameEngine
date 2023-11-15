
/**
 * @file log_system.h
 * @brief 日志封装
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

#ifndef SIMPLEGAMEENGINE_SRC_CORE_INCLUDE_LOG_LOG_SYSTEM_H
#define SIMPLEGAMEENGINE_SRC_CORE_INCLUDE_LOG_LOG_SYSTEM_H

#include <spdlog/spdlog.h>

namespace simple_game_engine {
namespace core {

class LogSystem {
 public:
  enum class LogLevel : uint8_t { debug, info, warn, error, fatal };

  LogSystem(const std::string &log_file_path, size_t lig_file_max_size,
            size_t log_file_max_count);
  ~LogSystem();

  template <typename... TARGS>
  void log(LogLevel level, TARGS &&...args) {
    switch (level) {
      case LogLevel::debug: {
        logger_->debug(std::forward<TARGS>(args)...);
        break;
      }
      case LogLevel::info: {
        logger_->info(std::forward<TARGS>(args)...);
        break;
      }
      case LogLevel::warn: {
        logger_->warn(std::forward<TARGS>(args)...);
        break;
      }
      case LogLevel::error: {
        logger_->error(std::forward<TARGS>(args)...);
        break;
      }
      case LogLevel::fatal: {
        logger_->critical(std::forward<TARGS>(args)...);
        fatalCallback(std::forward<TARGS>(args)...);
        break;
      }
      default: {
        break;
      }
    }
  }

  template <typename... TARGS>
  void info(TARGS &&...args) {
    logger_->info(std::forward<TARGS>(args)...);
  }

  template <typename... TARGS>
  void fatalCallback(TARGS &&...args) {
    const std::string format_str = fmt::format(std::forward<TARGS>(args)...);
    throw std::runtime_error(format_str);
  }

 private:
  std::shared_ptr<spdlog::logger> logger_;
};

}  // namespace core
}  // namespace simple_game_engine

#endif /* SIMPLEGAMEENGINE_SRC_CORE_INCLUDE_LOG_LOG_SYSTEM_H */
