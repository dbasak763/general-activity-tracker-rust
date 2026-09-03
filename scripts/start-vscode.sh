#!/bin/sh
set -eu

mongo_bin="work/mongodb/mongodb-macos-aarch64--8.0.26/bin/mongod"
mongo_data="work/live-mongo-data.sWhIf3"
mongo_log="work/vscode-mongodb.log"
container_name="activity-tracker-api-local"
image_name="rust-mongodb-activity-tracker-api:latest"

if ! nc -z 127.0.0.1 27018 2>/dev/null; then
  "$mongo_bin" \
    --dbpath "$mongo_data" \
    --port 27018 \
    --bind_ip 127.0.0.1 \
    --nounixsocket \
    --fork \
    --logpath "$mongo_log"
fi

if docker container inspect "$container_name" >/dev/null 2>&1; then
  docker start "$container_name" >/dev/null
else
  docker run --detach \
    --name "$container_name" \
    --restart unless-stopped \
    --publish 8080:8080 \
    --env APP_HOST=0.0.0.0 \
    --env APP_PORT=8080 \
    --env MONGODB_URI=mongodb://host.docker.internal:27018 \
    --env MONGODB_DATABASE=live_test_fresh_20260902 \
    --env RUST_LOG=activity_tracker=info,tower_http=info \
    "$image_name" >/dev/null
fi

attempt=1
while [ "$attempt" -le 30 ]; do
  if curl --fail --silent http://127.0.0.1:8080/health/ready; then
    printf '\nDashboard: http://127.0.0.1:8080/dashboard\n'
    exit 0
  fi
  attempt=$((attempt + 1))
  sleep 1
done

docker logs --tail 100 "$container_name"
exit 1
