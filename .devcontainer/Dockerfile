FROM rust:1-bookworm

RUN apt-get update \
    && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
      bash \
      ca-certificates \
      git \
      libasound2-dev \
      libgl1-mesa-dri \
      libudev-dev \
      libwayland-dev \
      libx11-dev \
      libxcb-render0-dev \
      libxcb-shape0-dev \
      libxcb-xfixes0-dev \
      libxcb1-dev \
      libxcursor-dev \
      libxi-dev \
      libxkbcommon-dev \
      libxkbcommon-x11-dev \
      libxrandr-dev \
      mesa-vulkan-drivers \
      pkg-config \
      xauth \
      xvfb \
    && ln -sf /usr/local/cargo/bin/cargo /usr/local/bin/cargo \
    && ln -sf /usr/local/cargo/bin/cargo-clippy /usr/local/bin/cargo-clippy \
    && ln -sf /usr/local/cargo/bin/cargo-fmt /usr/local/bin/cargo-fmt \
    && ln -sf /usr/local/cargo/bin/clippy-driver /usr/local/bin/clippy-driver \
    && ln -sf /usr/local/cargo/bin/rustc /usr/local/bin/rustc \
    && ln -sf /usr/local/cargo/bin/rustfmt /usr/local/bin/rustfmt \
    && mkdir -p /tmp/xdg-runtime \
    && chmod 700 /tmp/xdg-runtime \
    && rm -rf /var/lib/apt/lists/*

ENV PATH="/usr/local/cargo/bin:${PATH}"
ENV XDG_RUNTIME_DIR="/tmp/xdg-runtime"
