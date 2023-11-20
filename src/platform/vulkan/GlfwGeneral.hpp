
/**
 * @file window_system.h
 * @brief 窗口系统抽象
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

#ifndef SIMPLEGAMEENGINE_SRC_PLATFORM_VULKAN_2_H
#define SIMPLEGAMEENGINE_SRC_PLATFORM_VULKAN_2_H

#include "VKBase.h"
#define GLFW_INCLUDE_VULKAN
#include <GLFW/glfw3.h>

// 窗口的指针，全局变量自动初始化为NULL
GLFWwindow* pWindow;
// 显示器信息的指针
GLFWmonitor* pMonitor;
// 窗口标题
const char* windowTitle = "EasyVK";

bool InitializeWindow(VkExtent2D size, bool fullScreen = false,
                      bool isResizable = true, bool limitFrameRate = true) {
  // 首先用glfwInit(...)初始化GLFW，该函数在执行成功时返回true。
  if (!glfwInit()) {
    std::cout << "[ InitializeWindow ] ERROR\nFailed to initialize GLFW!\n";
    return false;
  }

  // 然后，在创建窗口前，必须调用glfwWindowHint(GLFW_CLIENT_API,
  // GLFW_NO_API)。GLFW同GLM一样最初是为OpenGL设计的，GLFW_CLIENT_API的默认设置是GLFW_OPENGL_API，这种情况下，GLFW会在创建窗口时创建OpenGL的上下文，这对于Vulkan而言是多余的，所以向GLFW说明不需要OpenGL的API。
  glfwWindowHint(GLFW_CLIENT_API, GLFW_NO_API);

  // 接着用glfwWindowHint(GLFW_RESIZABLE, isResizable)指定窗口可否拉伸：
  glfwWindowHint(GLFW_RESIZABLE, isResizable);

  uint32_t extensionCount = 0;
  const char** extensionNames;
  extensionNames = glfwGetRequiredInstanceExtensions(&extensionCount);
  if (!extensionNames) {
    std::cout
        << "[ InitializeWindow ]\nVulkan is not available on this machine!\n";
    glfwTerminate();
    return false;
  }
  for (size_t i = 0; i < extensionCount; i++)
    graphicsBase::Base().PushInstanceExtension(extensionNames[i]);

  // 然后便能用glfwCreateWindow(...)创建窗口了，该函数在执行成功时返回一非空指针。
  // 想必glfwCreateWindow(...)的前三个参数一目了然不必多说，其第四个参数用于指定全屏模式的显示器，若为nullptr则使用窗口模式，第五个参数可传入一个其它窗口的指针，用于与其他窗口分享内容。
  pWindow =
      glfwCreateWindow(size.width, size.height, windowTitle, nullptr, nullptr);

  // 如果你想实现全屏，则还需要用glfwGetPrimaryMonitor()取得当前显示器信息的指针，即便你在初始化时不想全屏，也有必要先取得它以备之后使用：
  pMonitor = glfwGetPrimaryMonitor();

  // 通常全屏时的图像区域应当跟屏幕分辨率一致，因此还需要使用glfwGetVideoMode(...)取得显示器当前的视频模式：
  const GLFWvidmode* pMode = glfwGetVideoMode(pMonitor);

  // 视频模式可能因为用户的操作而在程序运行过程中发生变更，因此总是在需要时获取，而不将其存储到全局变量。
  // 于是，若要实现全屏，根据fullScreen的值决定是否以全屏模式初始化窗口：
  pWindow = fullScreen ? glfwCreateWindow(pMode->width, pMode->height,
                                          windowTitle, pMonitor, nullptr)
                       : glfwCreateWindow(size.width, size.height, windowTitle,
                                          nullptr, nullptr);

  // 验证pWindow的值，若窗口创建失败，用glfwTerminate()来清理GLFW并让函数返回false：
  if (!pWindow) {
    std::cout << "[ InitializeWindow ]\nFailed to create a glfw window!\n";
    glfwTerminate();
    return false;
  }

  return true;
}

void TerminateWindow() { glfwTerminate(); }

void TitleFps() {
  static double time0 = glfwGetTime();
  static double time1;
  static double dt;
  static int dframe = -1;
  static std::stringstream info;
  time1 = glfwGetTime();
  dframe++;
  if ((dt = time1 - time0) >= 1) {
    info.precision(1);
    info << windowTitle << "    " << std::fixed << dframe / dt << " FPS";
    glfwSetWindowTitle(pWindow, info.str().c_str());
    info.str("");  // 别忘了在设置完窗口标题后清空所用的stringstream
    time0 = time1;
    dframe = 0;
  }
}

void MakeWindowFullScreen() {
  const GLFWvidmode* pMode = glfwGetVideoMode(pMonitor);
  glfwSetWindowMonitor(pWindow, pMonitor, 0, 0, pMode->width, pMode->height,
                       pMode->refreshRate);
}

void MakeWindowWindowed(VkOffset2D position, VkExtent2D size) {
  const GLFWvidmode* pMode = glfwGetVideoMode(pMonitor);
  glfwSetWindowMonitor(pWindow, nullptr, position.x, position.y, size.width,
                       size.height, pMode->refreshRate);
}

#endif  // SIMPLEGAMEENGINE_SRC_PLATFORM_VULKAN_2_H
