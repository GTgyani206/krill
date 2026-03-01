# krill  | Agentic Swarm Orchestrator

> ⚡ **Powered by [TinyFish](https://tinyfish.io)** — The Agentic Browser API
>
> **Terminal-native OSINT and Agentic Web Orchestration.**
> Standard terminal tools break on modern, dynamic websites. `krill` brings headless browser infrastructure natively to the command line, orchestrating parallel web agents to bypass anti-bot walls, manage React state, and synthesize market intelligence in real-time.

![krill demo](https://via.placeholder.com/800x400.png?text=Insert+Your+Terminal+Demo+GIF+Here)

## ⚡ The Problem
Analysts and developers spend hours writing fragile Puppeteer/Playwright scripts or paying for expensive API wrappers just to extract qualitative data from gated sites (Bloomberg, Reuters, Stripe). Standard `curl` requests fail immediately against Cloudflare and complex DOM structures.

##  The Solution
`krill` is a blazingly fast, concurrent orchestrator built in Rust. It utilizes the **TinyFish** API to spawn a swarm of headless browser agents that navigate complex web apps via natural language.

By passing the `--analyze` flag, `krill` transitions from a **Data Extractor** to an **Intelligence Synthesizer**, pipelining the aggregated JSON through an LLM to generate Wall Street-grade Markdown reports rendered directly in your terminal.

## ✨ Core Features
* **TinyFish Engine:** Unrestricted access to remote browser infrastructure with auto-configured anti-bot protection and state management.
* **Concurrent Swarm Architecture:** Powered by `tokio`, managing multiple async server-sent event streams simultaneously without blocking the main thread.
* **Network Resilience:** Built-in connection pooling and async semaphores strictly manage API rate limits and token budgets.
* **Master-Detail TUI:** A `btop`-inspired dashboard built with `ratatui`, featuring stateful scrolling, real-time agent telemetry, and unicode progress tracking.
* **Autonomous Synthesis:** Native `reqwest` integration with OpenAI to instantly reduce massive JSON haystacks into predictive, highly structured intelligence reports.
* **Rich Terminal Rendering:** Utilizes `termimad` to render the final synthesized intelligence brief natively in your standard output.

##  Quick Start

**1. Set your environment variables**
Create a `.env` file in the root directory:
```env
TINYFISH_API_KEY="your_tinyfish_api_key"
OPENAI_API_KEY="your_openai_api_key"
```

**2. Create a target list (`targets.txt`)**

```text
https://www.bloomberg.com/markets
https://www.reuters.com/markets/
https://finance.yahoo.com/topic/stock-market-news/
https://www.cnbc.com/markets/
https://coinmarketcap.com/headlines/news/
```

**3. Unleash the Swarm**

```bash
cargo run --release -- swarm --file targets.txt "Dismiss any popups. Locate the top 3 trending or breaking news articles. Extract the main headline, identify the primary company or technology mentioned, and determine the core sentiment (Bullish, Bearish, or Neutral). If a specific financial metric is mentioned, extract that exact figure." --analyze
```

## ️ Architecture Stack

* **Web Infrastructure:** [TinyFish API](https://tinyfish.io)
* **Language:** Rust (Strictly typed, memory-safe)
* **Async Runtime:** `tokio` (Channels, Semaphores, Mutexes)
* **Terminal UI:** `ratatui`, `crossterm`
* **Network:** `reqwest`, `reqwest-eventsource`
* **Intelligence:** OpenAI REST API
* **Rendering:** `termimad`
