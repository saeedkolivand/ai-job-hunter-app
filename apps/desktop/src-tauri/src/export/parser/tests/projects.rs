use crate::export::parser::is_project_stack_shaped;

/// `is_project_stack_shaped` must accept a two-item stack (ONE separator, which
/// `is_contact_shaped` misses) and reject the prose that shares its slot.
#[test]
fn project_stack_shape_accepts_stacks_and_rejects_prose() {
    for stack in [
        "Rust · SQLite · Clap",
        "Rust · SQLite",
        "TypeScript | React | Vite",
        "Tauri 2 · Rust · React 19 · TypeScript · Zod",
    ] {
        assert!(is_project_stack_shaped(stack), "must accept {stack:?}");
    }

    // A stack may carry a dotted package name; every bare-domain heuristic that
    // rejects `demo.example.dev` also rejects these, so they pin the line.
    for stack in [
        "Node.js · socket.io · Express",
        "TypeScript · React · GitHub Actions",
    ] {
        assert!(is_project_stack_shaped(stack), "must accept {stack:?}");
    }

    for prose in [
        // Contact content in the slot directly under a project title. `URL_RE`
        // alone does NOT catch the first of these — its scheme arm is anchored at
        // the start of the line — which is why the predicate tests `://` too.
        "Demo · https://example.dev",
        "Support · +1 555 0100",
        "github.com/x/y · demo.example.dev",
        "Docs · [handbook](https://example.dev/handbook)",
        "Mirror · www.example.dev",
        // No separator at all — an ordinary description line.
        "A double-entry bookkeeping tool for the terminal.",
        // Separators, but it is a sentence.
        "Ships on Windows · macOS · Linux.",
        // A trailing colon introduces prose that follows.
        "Built with · tested against:",
        // A contact line that wandered into the section.
        "jane@example.com · https://janedoe.dev",
        // Long enough to be prose no matter how many separators it holds.
        "Designed and shipped the ingest layer · the scheduler · the reporting \
         API · and the operator console for a distributed rate limiter",
        "",
        "   ",
    ] {
        assert!(!is_project_stack_shaped(prose), "must reject {prose:?}");
    }
}
