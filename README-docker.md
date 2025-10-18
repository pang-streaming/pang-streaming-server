# Docker Compose 사용 가이드

이 프로젝트는 Docker Compose를 사용하여 쉽게 배포할 수 있습니다.

## 파일 구조

- `docker-compose.yml`: 통합 설정 파일
- `deploy/configs/`: 모든 설정 파일들
- `deploy/scripts/`: 배포 스크립트들
- `.dockerignore`: Docker 빌드 시 제외할 파일들

## 사용법

### 서비스 실행

```bash
# 모든 서비스 실행
docker-compose up -d

# 로그 확인
docker-compose logs -f pang-streaming-server

# 서비스 중지
docker-compose down
```

### 스크립트 사용

```bash
# 서비스 시작
./deploy/scripts/start.sh

# 서비스 중지
./deploy/scripts/stop.sh

# 서비스 재시작
./deploy/scripts/restart.sh
```

## 서비스 구성

### pang-streaming-server
- **포트**: 1935 (RTMP), 8080 (HTTP API), 1930 (추가)
- **볼륨**: `./hls_output` → `/app/hls_output`
- **환경변수**: RUST_LOG, RUST_BACKTRACE

### nginx
- **포트**: 80 (HTTP), 443 (HTTPS)
- **역할**: HLS 스트리밍 파일 서빙 및 API 프록시
- **볼륨**: `./hls_output` → `/usr/share/nginx/html/hls`

## 네트워크

- `pang-network`: 모든 서비스가 통신하는 브리지 네트워크

## 볼륨

- `hls_output`: HLS 세그먼트 파일 저장용
- `redis_data`: Redis 데이터 저장용

## 서비스 구성

### pang-streaming-server
- **포트**: 1935 (RTMP), 8080 (HTTP API), 1930 (추가)
- **볼륨**: `./hls_output` → `/app/hls_output`
- **환경변수**: RUST_LOG, RUST_BACKTRACE

### xiu
- **포트**: 1936 (RTMP), 8081 (HTTPFLV), 8083 (WebRTC)
- **네트워크**: host 모드 (호스트 네트워크 직접 사용)
- **볼륨**: `./deploy/logs` → `/app/logs`
- **특권**: privileged 모드로 실행

### nginx
- **포트**: 80 (HTTP), 443 (HTTPS)
- **역할**: HLS 스트리밍 파일 서빙 및 API 프록시
- **볼륨**: `./hls_output` → `/usr/share/nginx/html/hls`

## 서비스 특징

- **통합 구성**: 하나의 docker-compose.yml 파일로 모든 서비스 관리
- **Redis 지원**: 세션 관리 및 캐싱을 위한 Redis 포함
- **로그 관리**: deploy/logs 디렉토리에서 중앙 집중식 로그 관리
- **설정 관리**: deploy/configs 디렉토리에서 모든 설정 파일 관리

## 문제 해결

### 포트 충돌
```bash
# 사용 중인 포트 확인
netstat -tulpn | grep :1935
netstat -tulpn | grep :8080

# 다른 포트로 변경하려면 docker-compose.yml 수정
```

### 볼륨 권한 문제
```bash
# HLS 출력 디렉토리 권한 설정
sudo chown -R $USER:$USER ./hls_output
chmod -R 755 ./hls_output
```

### 로그 확인
```bash
# 모든 서비스 로그
docker-compose logs

# 특정 서비스 로그
docker-compose logs pang-streaming-server
docker-compose logs xiu
docker-compose logs nginx
docker-compose logs redis

# 파일 로그 확인
tail -f deploy/logs/xiu.log
```
