# krill

CLI for the [TinyFish](https://tinyfish.app) web automation API.

## Installation

```sh
cargo install --path .
```

## Usage

```sh
# Set your API key
export TINYFISH_API_KEY="your-api-key"

# Run an automation task
krill run <URL> "<GOAL>"
```

You can also pass the key directly:

```sh
krill --api-key <KEY> run "https://example.com" "Extract the page title"
```
