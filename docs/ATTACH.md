# Dynamic Attach (Agent_OnAttach)

This crate supports dynamic attach via the `Agent_OnAttach` entry point.
Implement `Agent::on_attach` and use `export_agent!` — the macro generates
the correct native entry points automatically.

## Minimal Example

```rust,ignore
use jvmti_bindings::prelude::*;

#[derive(Default)]
struct AttachLogger;

impl Agent for AttachLogger {
    fn on_attach(&self, context: AgentLoadContext<'_>) -> jni::jint {
        println!("[AttachLogger] attached with options: {:?}", context.options_lossy());
        let Ok(_jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        jni::JNI_OK
    }
}

export_agent!(AttachLogger);
```

## Notes

- `on_attach` is called for every Attach API load request, including requests
  made after the same native agent library has already been loaded.
- `export_agent!` constructs one process-global agent and reuses it for startup
  load and every subsequent attach. Initialization in `on_attach` must therefore
  be idempotent, and agent state must tolerate concurrent attach requests.
- `AgentLoadContext::option_bytes` preserves the exact option bytes.
  `options_str` validates Java Modified UTF-8, while `options_lossy` is the
  explicit replacement-decoding convenience.
- You can request capabilities and enable JVMTI events inside `on_attach`.
- Thread and JNI safety rules still apply (see `docs/SAFETY.md`).
- JEP 451 warns for dynamic agent loading and permits a future default denial;
  operators should use `-XX:+EnableDynamicAgentLoading` where required.
- Startup loading with `-agentpath` remains the preferred unattended deployment.

The repository proof `scripts/prove-repeated-attach-live.sh` starts an agent,
attaches it twice with distinct options, and verifies that one agent instance
receives all lifecycle calls.

`scripts/prove-attach-policy-live.sh` proves three separate contracts on
supported modern runtimes: startup `-agentpath` loading still succeeds when
dynamic loading is disabled, an Attach API load is rejected when
`-XX:-EnableDynamicAgentLoading` is explicit, and the same load succeeds when
`-XX:+EnableDynamicAgentLoading` is explicit.
