# ledmoji-mqtt-image-publisher

## Docker

### Build Image
```bash	
docker build -t ledmoji-daemon:latest .
```

```bash
docker buildx create --driver=docker-container --name=container
docker buildx build --builder=container --platform=linux/x86_64 -t andreban/ledmoji-daemon --push .

```
### Run Container
```bash
docker run -d --name ledmoji-daemon --restart unless-stopped ledmoji-daemon:latest
```