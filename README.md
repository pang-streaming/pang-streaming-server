# pang rust streaming server

## docker build
```shell
docker build -t pang-streaming-server:latest .
```
### docker run
```shell
docker run -d \
  --name pang-streaming-server \
  -p 1930:1930 \
  -p 8081:8081 \
  pang-streaming-server:latest
```