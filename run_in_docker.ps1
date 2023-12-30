# Replace 'your_container_name' with the actual name of your Docker container
$containerName = "pico-webapp-rust-cross-compile-1"

# Command to execute in the Docker container
$command = "./compile-pico.sh"

# Executing the command in the Docker container
docker exec $containerName $command