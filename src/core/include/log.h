
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

#include <spdlog/spdlog.h>
#include <sys/time.h>

#include "config.h"

extern std::shared_ptr<spdlog::logger> SRLOG;

/// @todo 修复 clang-tidy

void log_init(void);

#endif /* SIMPLEGAMEENGINE_LOG_H */
