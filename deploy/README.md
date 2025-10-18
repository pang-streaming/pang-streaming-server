# Deploy 디렉토리 구조

이 디렉토리는 스트리밍 서버의 배포 관련 파일들을 관리합니다.

## 디렉토리 구조

```
deploy/
├── configs/           # 설정 파일들
│   ├── config.toml    # 메인 애플리케이션 설정
│   ├── xiu-config.toml # Xiu RTMP 서버 설정
│   └── nginx.conf     # Nginx 설정
├── logs/              # 로그 파일들
│   └── (로그 파일들이 여기에 저장됩니다)
└── scripts/           # 배포 스크립트들
    ├── start.sh       # 서비스 시작 스크립트
    ├── stop.sh        # 서비스 중지 스크립트
    └── restart.sh     # 서비스 재시작 스크립트
```

## 설정 파일 설명

### config.toml
- 메인 Rust 애플리케이션의 설정 파일
- 서버 포트, HLS 설정, S3 설정 등을 포함

### xiu-config.toml
- Xiu RTMP 서버의 설정 파일
- RTMP, RTSP, WebRTC, HLS 등의 프로토콜 설정
- 인증 및 보안 설정

### nginx.conf
- Nginx 웹 서버 설정
- HLS 스트리밍 파일 서빙
- CORS 헤더 설정
- API 프록시 설정

## 사용법

### 서비스 시작
```bash
./deploy/scripts/start.sh
```

### 서비스 중지
```bash
./deploy/scripts/stop.sh
```

### 서비스 재시작
```bash
./deploy/scripts/restart.sh
```

## 포트 정보

- **1935**: RTMP 포트 (pang-streaming-server)
- **1936**: RTMP 포트 (xiu 서버)
- **8080**: HTTP API 포트
- **8081**: HTTPFLV 포트 (xiu)
- **8083**: WebRTC 포트 (xiu)
- **80**: Nginx HTTP 포트
- **443**: Nginx HTTPS 포트

## 로그 확인

로그 파일들은 `deploy/logs/` 디렉토리에 저장됩니다:

```bash
# 실시간 로그 확인
tail -f deploy/logs/xiu.log

# 모든 로그 파일 확인
ls -la deploy/logs/
```

## 설정 변경

설정 파일을 변경한 후에는 서비스를 재시작해야 합니다:

```bash
./deploy/scripts/restart.sh
```

