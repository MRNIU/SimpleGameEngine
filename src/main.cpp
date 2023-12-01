
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

#include <iostream>
#include <string>
#include <string_view>

#include "platform/file_system/path.h"
#include "platform/rhi/bgfx.h"
#include "platform/rhi/imgui_impl_bgfx.h"
#include "platform/window_system/imgui_sdl2.h"
#include "shader/shaders.inc"
#include "utils/config/config.h"
#include "utils/log/log_system.h"

struct PosColorVertex {
  float x;
  float y;
  float z;
  uint32_t abgr;
};

static PosColorVertex cube_vertices[] = {
    {-1.0f, 1.0f, 1.0f, 0xff000000},   {1.0f, 1.0f, 1.0f, 0xff0000ff},
    {-1.0f, -1.0f, 1.0f, 0xff00ff00},  {1.0f, -1.0f, 1.0f, 0xff00ffff},
    {-1.0f, 1.0f, -1.0f, 0xffff0000},  {1.0f, 1.0f, -1.0f, 0xffff00ff},
    {-1.0f, -1.0f, -1.0f, 0xffffff00}, {1.0f, -1.0f, -1.0f, 0xffffffff},
};

static const uint16_t cube_tri_list[] = {
    0, 1, 2, 1, 3, 2, 4, 6, 5, 5, 6, 7, 0, 2, 4, 4, 2, 6,
    1, 5, 3, 5, 7, 3, 0, 4, 1, 4, 5, 1, 2, 3, 6, 6, 3, 7,
};

struct context_t {
  SDL_Window* window = nullptr;
  bgfx::ProgramHandle program = BGFX_INVALID_HANDLE;
  bgfx::VertexBufferHandle vbh = BGFX_INVALID_HANDLE;
  bgfx::IndexBufferHandle ibh = BGFX_INVALID_HANDLE;

  float cam_pitch = 0.0f;
  float cam_yaw = 0.0f;
  float rot_scale = 0.01f;

  int prev_mouse_x = 0;
  int prev_mouse_y = 0;

  int width = 0;
  int height = 0;

  bool quit = false;
};

void main_loop(void* data) {
  auto context = static_cast<context_t*>(data);

  for (SDL_Event current_event; SDL_PollEvent(&current_event) != 0;) {
    ImGui_ImplSDL2_ProcessEvent(&current_event);
    if (current_event.type == SDL_QUIT) {
      context->quit = true;
      break;
    }
  }

  ImGui_Implbgfx_NewFrame();
  ImGui_ImplSDL2_NewFrame();

  ImGui::NewFrame();
  ImGui::ShowDemoWindow();  // your drawing here
  ImGui::Render();
  ImGui_Implbgfx_RenderDrawLists(ImGui::GetDrawData());

  if (!ImGui::GetIO().WantCaptureMouse) {
    // simple input code for orbit camera
    int mouse_x, mouse_y;
    const int buttons = SDL_GetMouseState(&mouse_x, &mouse_y);
    if ((buttons & SDL_BUTTON(SDL_BUTTON_LEFT)) != 0) {
      int delta_x = mouse_x - context->prev_mouse_x;
      int delta_y = mouse_y - context->prev_mouse_y;
      context->cam_yaw += float(-delta_x) * context->rot_scale;
      context->cam_pitch += float(-delta_y) * context->rot_scale;
    }
    context->prev_mouse_x = mouse_x;
    context->prev_mouse_y = mouse_y;
  }

  float cam_rotation[16];
  bx::mtxRotateXYZ(cam_rotation, context->cam_pitch, context->cam_yaw, 0.0f);

  float cam_translation[16];
  bx::mtxTranslate(cam_translation, 0.0f, 0.0f, -5.0f);

  float cam_transform[16];
  bx::mtxMul(cam_transform, cam_translation, cam_rotation);

  float view[16];
  bx::mtxInverse(view, cam_transform);

  float proj[16];
  bx::mtxProj(proj, 60.0f, float(context->width) / float(context->height), 0.1f,
              100.0f, bgfx::getCaps()->homogeneousDepth);

  bgfx::setViewTransform(0, view, proj);

  float model[16];
  bx::mtxIdentity(model);
  bgfx::setTransform(model);

  bgfx::setVertexBuffer(0, context->vbh);
  bgfx::setIndexBuffer(context->ibh);

  bgfx::submit(0, context->program);

  bgfx::frame();
}

const int width = 800;
const int height = 600;

auto main(int, char**) -> int {
  auto config_file_path =
      simple_game_engine::platform::Path::GetExecutablePath()
          .parent_path()
          .append("config.json");
  simple_game_engine::utils::Config config(config_file_path);
  simple_game_engine::utils::LogSystem log_system(config.GetLogFilePath(),
                                                  config.GetLogFileMaxSize(),
                                                  config.GetLogFileMaxCount());

  SPDLOG_INFO("加载配置文件: {}", config_file_path.string());

  simple_game_engine::platform::SDL2 sdl2(width, height);

  bgfx::renderFrame();  // single threaded mode

  bgfx::PlatformData pd{};
#if BX_PLATFORM_WINDOWS
  pd.nwh = sdl2.wmi.info.win.window;
#elif BX_PLATFORM_OSX
  pd.nwh = sdl2.wmi.info.cocoa.window;
#elif BX_PLATFORM_LINUX
  pd.ndt = sdl2.wmi.info.x11.display;
  pd.nwh = (void*)(uintptr_t)sdl2.wmi.info.x11.window;
#endif  // BX_PLATFORM_WINDOWS ? BX_PLATFORM_OSX ? BX_PLATFORM_LINUX

  bgfx::Init bgfx_init;
  bgfx_init.type = bgfx::RendererType::Count;  // auto choose renderer
  bgfx_init.resolution.width = width;
  bgfx_init.resolution.height = height;
  bgfx_init.resolution.reset = BGFX_RESET_VSYNC;
  bgfx_init.platformData = pd;
  bgfx::init(bgfx_init);

  bgfx::setViewClear(0, BGFX_CLEAR_COLOR | BGFX_CLEAR_DEPTH, 0x6495EDFF, 1.0f,
                     0);
  bgfx::setViewRect(0, 0, 0, width, height);

  ImGui::CreateContext();

  ImGui_Implbgfx_Init(255);
#if BX_PLATFORM_WINDOWS
  ImGui_ImplSDL2_InitForD3D(sdl2.window);
#elif BX_PLATFORM_OSX
  ImGui_ImplSDL2_InitForMetal(sdl2.window);
#elif BX_PLATFORM_LINUX
  ImGui_ImplSDL2_InitForOpenGL(sdl2.window, nullptr);
#endif  // BX_PLATFORM_WINDOWS ? BX_PLATFORM_OSX ? BX_PLATFORM_LINUX

  bgfx::VertexLayout pos_col_vert_layout;
  pos_col_vert_layout.begin()
      .add(bgfx::Attrib::Position, 3, bgfx::AttribType::Float)
      .add(bgfx::Attrib::Color0, 4, bgfx::AttribType::Uint8, true)
      .end();
  bgfx::VertexBufferHandle vbh = bgfx::createVertexBuffer(
      bgfx::makeRef(cube_vertices, sizeof(cube_vertices)), pos_col_vert_layout);
  bgfx::IndexBufferHandle ibh = bgfx::createIndexBuffer(
      bgfx::makeRef(cube_tri_list, sizeof(cube_tri_list)));

  bgfx::RendererType::Enum type = bgfx::getRendererType();
  /// @bug bgfx fatal error
  bgfx::ProgramHandle program = bgfx::createProgram(
      bgfx::createEmbeddedShader(simple_game_engine::shader::kEmbeddedShaders,
                                 type, "vs"),
      bgfx::createEmbeddedShader(simple_game_engine::shader::kEmbeddedShaders,
                                 type, "fs"),
      true);

  context_t context;
  context.width = width;
  context.height = height;
  context.program = program;
  context.window = sdl2.window;
  context.vbh = vbh;
  context.ibh = ibh;

  while (!context.quit) {
    main_loop(&context);
  }

  bgfx::destroy(vbh);
  bgfx::destroy(ibh);
  bgfx::destroy(program);

  ImGui_ImplSDL2_Shutdown();
  ImGui_Implbgfx_Shutdown();

  ImGui::DestroyContext();
  bgfx::shutdown();

  return 0;
}
