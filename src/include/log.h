
/**
 * @file log.h
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

#ifndef SIMPLEGAMEENGINE_LOG_H
#define SIMPLEGAMEENGINE_LOG_H

#include <sys/time.h>

#include <spdlog/spdlog.h>

#include "config.h"

extern std::shared_ptr<spdlog::logger> SRLOG;

/// @todo 修复 clang-tidy

/// 微秒到秒
static constexpr uint32_t US2S = 1000000;

/**
 * 获取当前微秒数，用于性能分析
 * @return auto                 当前微秒
 */
inline auto us() {
  struct timeval t = {};

  gettimeofday(&t, nullptr);
  return t.tv_sec * 1000000 + t.tv_usec;
}

void log_init(void);

#endif /* SIMPLEGAMEENGINE_LOG_H */
