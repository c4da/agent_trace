# Model sizing and troubleshooting

## HTTP 500 "CUDA error: out of memory"

The model does not fit in GPU memory. The weights plus context cache must fit in VRAM
unless the runtime offloads layers to CPU, which is much slower.

Rough rule for 4-bit quantised (Q4) models: about 0.6 GB of VRAM per billion parameters,
plus 1–2 GB for context and runtime.

| GPU VRAM | Comfortable Q4 model size |
| :--- | :--- |
| 6–8 GB | up to ~7–8B |
| 12 GB | up to ~14B |
| 16 GB | up to ~20B |
| 24 GB | up to ~32B |

Use `ollama ps` to see what is loaded and `ollama list` for on-disk sizes. If a model fails
with out-of-memory, pick a smaller one, or close other GPU-heavy programs (games, renderers,
browsers with hardware acceleration).

## Heavy models and memory hygiene

After running a large model, free GPU memory for other work:

```bash
curl http://127.0.0.1:11434/api/generate -d '{"model": "<model>", "keep_alive": 0}'
```

AgentTrace does this automatically for models whose names contain `27b` or `32b`.

## `fallback_used: true`

The preferred node or model failed or timed out and the gateway retried on a local fallback
model. The answer comes from a different, usually smaller model. Say so in your report.

## Timeouts

Large prompts on small GPUs can take a minute or more per stage. If dispatches time out,
send less code per request.

## Generic or off-topic answers

Almost always a prompt problem: the code was not included, it was truncated by the context
window, or the domain was not stated. See step 2 of the skill.
