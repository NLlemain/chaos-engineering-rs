# AI API Failure Pack

Provider-aware HTTP and streaming faults for OpenAI, Azure OpenAI, Anthropic, Gemini, OpenRouter, Ollama, Mistral, Groq, Cohere, Together, vLLM, and generic APIs.

Run a scenario, then point the client SDK's base URL at the scenario's local `listen` address. Existing authentication headers and request paths are forwarded to the real provider.

The OpenAI-compatible profile can also target Azure OpenAI, Mistral, Groq, Together, vLLM, and other compatible servers by changing `upstream` and `provider`.

Only use these scenarios with development or staging credentials.
