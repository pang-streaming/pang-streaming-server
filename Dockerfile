# 최신 Rust 버전 사용
FROM rust:1.75-bullseye

# GStreamer 및 필요한 라이브러리 설치
RUN apt-get update && apt-get install -y \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libgstreamer-plugins-bad1.0-dev \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    gstreamer1.0-tools \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 작업 디렉토리 설정
WORKDIR /app

# Cargo 파일들 먼저 복사 (캐싱 최적화)
COPY Cargo.toml Cargo.lock ./

# 더미 소스로 의존성 빌드 (캐싱용)
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src target/release/deps/pang*

# 실제 소스 코드 복사
COPY . .

# 애플리케이션 빌드
RUN cargo build --release

# 포트 노출
EXPOSE 1930

# HLS 출력 디렉토리 생성
RUN mkdir -p /app/hls_output

# 애플리케이션 실행
CMD ["./target/release/pang-streaming-server"]