# 최신 Rust 버전 사용
FROM rust:latest

# FFmpeg 및 필요한 라이브러리 설치
RUN apt-get update && apt-get install -y \
    ffmpeg \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 작업 디렉토리 설정
WORKDIR /app

# Cargo 파일들 먼저 복사 (캐싱 최적화)
COPY Cargo.toml ./
COPY Cargo.lock ./

# 더미 소스로 의존성 빌드 (캐싱용)
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# 실제 소스 코드 복사
COPY src ./src

# 애플리케이션 빌드
RUN cargo build --release

# 포트 노출
EXPOSE 1930
EXPOSE 1935

# HLS 출력 디렉토리 생성
RUN mkdir -p /app/hls_output

# 디버깅을 위한 환경 변수 설정
ENV RUST_LOG=debug
ENV RUST_BACKTRACE=full

# 애플리케이션 실행 (디버깅용)
CMD ["sh", "-c", "echo 'Starting pang-streaming-server...' && ls -la /app && echo 'Config file:' && cat /app/config.toml && echo 'Running app...' && ./target/release/pang-streaming-server"]