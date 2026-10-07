# 📚 Chapter 3: Structs, Enums & Pattern Matching

***

Hello, fellow adventurer! Welcome back to our journey through the world of Rust.

In Chapter 2, we learned how to handle basic data and control the flow of our programs. Now, we are going to move from handling simple variables to building *complex, meaningful data structures*.

If variables are single bricks, then **Structs** and **Enums** are the blueprints for entire buildings. They allow us to group related pieces of information together, making our code incredibly organized and readable.

This chapter is arguably one of the most fun, because it’s where Rust starts to feel like a truly powerful, expressive language. Let's dive in!

---

## 🧱 3.1 Custom Data Types: Structs

A `struct` (short for structure) is a way to create a custom, named data type that groups together several related fields. Think of it like a blueprint for a specific object in the real world—like a `User`, a `Point`, or, in our case, a `TraceEvent`.

### 🏗️ Classic Structs: Grouping Related Fields

Let's define a `TraceEvent`. This event needs an ID, a target system, and how long it took.

```rust
// Define the blueprint for our TraceEvent
struct TraceEvent {
    event_id: String,
    target: String,
    duration_ms: f64,
}

fn main() {
    // 1. Building a struct instance (creating an object from the blueprint)
    let event1 = TraceEvent {
        event_id: String::from("E-401"),
        target: String::from("DatabaseService"),
        duration_ms: 12.5,
    };

    println!("Event ID: {}", event1.event_id);
    println!("Target: {}", event1.target);
}
```

In the code above, `TraceEvent` is the *type*, and `event1` is an *instance* of that type.

### ✨ Shorthand and Update Syntax

When you are creating many instances of the same struct, typing out every field can get repetitive. Rust offers two fantastic shortcuts:

#### 1. Field Init Shorthand
If all the field names in your struct match the variable names you are using, you can skip writing the field name:

```rust
// Notice we don't write 'event_id: String, target: String, ...'
let event2 = TraceEvent {
    event_id: String::from("E-402"),
    target: String::from("APIGateway"),
    duration_ms: 5.1,
};
```

#### 2. Struct Update Syntax (`..other`)
This is a lifesaver! If you want to create a new instance that is almost identical to an existing one, but just needs one or two fields changed, use the update syntax (`..other`).

```rust
let base_event = TraceEvent {
    event_id: String::from("E-001"),
    target: String::from("AuthService"),
    duration_ms: 10.0,
};

// Create a new event, but copy everything from base_event, 
// and only change the ID and duration.
let updated_event = TraceEvent {
    event_id: String::from("E-002"), // <-- Overridden
    duration_ms: 15.0,             // <-- Overridden
    ..base_event,                  // <-- Copies target: "AuthService"
};

println!("Updated Target: {}", updated_event.target); // Output: AuthService
```

### 🛠️ Adding Methods with `impl`

A struct is just data. To make it useful, we need to give it *behavior*—functions that operate on its data. In Rust, we do this using an `impl` (implementation) block.

We can add methods that read the data, change the data, or even create new instances.

```rust
struct TraceEvent {
    event_id: String,
    target: String,
    duration_ms: f64,
}

// The 'impl' block defines methods for the TraceEvent type
impl TraceEvent {
    // 1. Associated Function (Constructor)
    // This function belongs to the type itself, not a specific instance.
    // It's often used as a factory or constructor (like 'new()' in other languages).
    fn new(event_id: String, target: String, duration_ms: f64) -> TraceEvent {
        TraceEvent {
            event_id,
            target,
            duration_ms,
        }
    }

    // 2. Method using &self (Read-only access)
    // The '&self' means the method borrows the instance immutably. 
    // It can read the data, but not change it.
    fn get_summary(&self) -> String {
        format!(
            "Event {} completed on {} in {:.1}ms.",
            self.event_id, self.target, self.duration_ms
        )
    }

    // 3. Method using &mut self (Mutable access)
    // The '&mut self' means the method borrows the instance mutably. 
    // It can read AND change the data.
    fn mark_processed(&mut self) {
        println!("--- Event {} has been marked as processed! ---", self.event_id);
        // In a real scenario, we might change a 'processed: bool' field here.
    }
}

fn main() {
    // Use the associated function (constructor)
    let mut event = TraceEvent::new(
        String::from("E-999"), 
        String::from("Logger"), 
        2.2
    );

    // Call the read-only method
    println!("{}", event.get_summary());

    // Call the mutable method (requires 'event' to be declared 'mut')
    event.mark_processed();
}
```

**Quick Cheat Sheet for `impl`:**

| Signature | Meaning | Use Case |
| :--- | :--- | :--- |
| `fn method(&self)` | Immutable borrow | Reading data (e.g., `get_name()`) |
| `fn method(&mut self)` | Mutable borrow | Changing data (e.g., `increment_counter()`) |
| `fn Self::new(...)` | Associated function | Creating a new instance (Constructor) |

---

## 🌈 3.2 Enums: More Powerful Than in Any Other Language

If structs are blueprints for *data*, **Enums** (Enumerations) are blueprints for *possibilities*.

In many languages, an enum is just a list of named constants (e.g., `Color::Red`, `Color::Blue`). But in Rust, enums can hold rich data, making them incredibly powerful for modeling complex states.

### 🚦 Basic Enums

A basic enum simply lists possible variants:

```rust
enum SecurityVerdict {
    Allowed,
    Flagged,
    Blocked,
}

fn check_verdict(verdict: SecurityVerdict) {
    match verdict {
        SecurityVerdict::Allowed => println!("✅ Access granted."),
        SecurityVerdict::Flagged => println!("⚠️ Requires manual review."),
        SecurityVerdict::Blocked => println!("❌ Access denied."),
    }
}
```

### 📦 Enums Holding Rich Data (The Power!)

This is where Rust shines. An enum variant can carry its own data, like a mini-struct!

Let's model different types of events our agent might encounter:

```rust
// Enum 1: Defines the type of action taken
enum EventType {
    FileRead,
    FileWrite,
    CommandRun,
    LlmCompletion { model_name: String, tokens_used: u32 }, // Rich data!
}

// Enum 2: Defines the result of a network call
enum NetworkResult {
    Success(u16), // Carries a status code
    Failure(String), // Carries an error message
}

fn main() {
    // Example 1: A simple event
    let read_event = EventType::FileRead;
    
    // Example 2: A rich event (LLM completion)
    let llm_event = EventType::LlmCompletion { 
        model_name: String::from("GPT-4"), 
        tokens_used: 512 
    };

    // Example 3: A network failure
    let net_fail = NetworkResult::Failure(String::from("Timeout"));
}
```

When you use a rich enum, you are not just saying "it's an `LlmCompletion`"; you are saying, "it's an `LlmCompletion` *and* it has a specific `model_name` and `tokens_used`." This is incredibly precise modeling!

---

## 🚫 3.3 Eliminating Null: The `Option<T>` Enum

Before we get to the most powerful tool, we need to talk about a historical programming disaster: **null pointers**.

In many older languages, if a function might not return a value (e.g., a search function might not find anything), it returns `null`. This is the "Billion-Dollar Mistake" mentioned by Tony Hoare—a mistake because it leads to runtime crashes (Null Pointer Exceptions) that are incredibly hard to debug.

Rust says: **"No nulls allowed."**

Instead, Rust uses the `Option<T>` enum to force you to deal with the possibility of missing data *at compile time*.

`Option<T>` has only two variants:

1.  `Some(T)`: The value exists! It holds a value of type `T`.
2.  `None`: The value does not exist.

```rust
// Option<i32> means this type either holds an i32 or it holds nothing.
let maybe_age: Option<i32> = Some(30);
let missing_data: Option<String> = None;

fn main() {
    // If we try to access the value without checking, Rust will complain!
    // println!("{}", maybe_age.unwrap()); // This works for Some(T)

    // If we try to unwrap None, the program panics!
    // println!("{}", missing_data.unwrap()); // 💥 CRASH!
}
```

By using `Option<T>`, Rust forces you, the programmer, to write code that explicitly handles both the `Some` case and the `None` case. This eliminates entire classes of runtime errors!

---

## 🧩 3.4 The Power of Pattern Matching

If structs and enums are the data, **Pattern Matching** is the logic that lets us interact with that data safely and powerfully.

The `match` expression is Rust's ultimate control flow tool. It allows you to check a value against a series of patterns and execute code based on which pattern matches.

### 🎯 Exhaustive Checking with `match`

The most beautiful feature of `match` is that the compiler *forces* you to cover every possible case. This is called **exhaustive checking**. If you forget a variant, the compiler yells at you!

Let's use our `NetworkResult` enum:

```rust
enum NetworkResult {
    Success(u16),
    Failure(String),
}

fn process_network_result(result: NetworkResult) {
    // We match against the enum variants
    match result {
        // Case 1: Success. We use pattern matching to extract the status code (u16).
        NetworkResult::Success(code) => {
            println!("✅ Network call succeeded with status code: {}", code);
        }
        // Case 2: Failure. We extract the error message (String).
        NetworkResult::Failure(message) => {
            println!("❌ Network failed! Error: {}", message);
        }
    }
}

fn main() {
    let success = NetworkResult::Success(200);
    let failure = NetworkResult::Failure(String::from("DNS resolution failed"));

    process_network_result(success);
    process_network_result(failure);
}
```

Notice how `NetworkResult::Success(code)` doesn't just match the variant; it *binds* the value inside the variant to a new variable called `code`. This is data extraction in action!

### 💡 The Concise `if let` Syntax

Sometimes, you only care about *one* specific pattern, and you don't want to write a full `match` block. For these simpler checks, Rust provides `if let`.

`if let` is syntactic sugar for a `match` statement that only cares about one pattern.

Let's use `Option<T>` and `if let`:

```rust
fn main() {
    let maybe_user_id: Option<u32> = Some(101);
    let no_user_id: Option<u32> = None;

    // We only care about the case where the ID exists (Some(id)).
    if let Some(id) = maybe_user_id {
        println!("Found user with ID: {}", id);
    } else {
        println!("User ID was missing.");
    }

    // The same logic for the None case:
    if let Some(id) = no_user_id {
        println!("This won't print.");
    } else {
        println!("Correctly handled missing ID.");
    }
}
```

---

## 🚀 Practice Exercises: Agent Telemetry

Let's put everything we've learned together. We are going to model a complete telemetry event for our agent.

**Goal:** Define the necessary structs and enums, and then write a function that processes the event based on its type and outcome.

### Task 1: Define the Data Structures

1.  Define a struct called `TelemetryEvent` that holds:
    *   `event_type`: An `EventType` (Enum).
    *   `timestamp`: A `u64` (Unix timestamp).
    *   `is_critical`: A `bool`.

2.  Define an enum called `EventType` with the following variants:
    *   `SystemCheck` (No data)
    *   `FileOperation { path: String, bytes_read: u64 }` (Rich data)
    *   `NetworkCall { url: String, success: Option<u16> }` (Rich data, using `Option<T>`)

### Task 2: Implement the Processor

1.  Write a function `log_event(event: TelemetryEvent)` that uses a `match` statement on the `event.event_type`.
2.  Inside the `match`, use pattern matching to:
    *   Print a message for `SystemCheck`.
    *   Extract the `path` and `bytes_read` for `FileOperation` and print them.
    *   Handle `NetworkCall`:
        *   If `success` is `Some(code)`, print a success message including the code.
        *   If `success` is `None`, print a failure message.

### Solution Template (Try to solve it before looking!)

```rust
// --- START OF YOUR CODE HERE ---

// 1. Define the Enum
enum EventType {
    SystemCheck,
    FileOperation { path: String, bytes_read: u64 },
    NetworkCall { url: String, success: Option<u16> },
}

// 2. Define the Struct
struct TelemetryEvent {
    event_type: EventType,
    timestamp: u64,
    is_critical: bool,
}

// 3. Implement the Processor
fn log_event(event: TelemetryEvent) {
    println!("\n--- Processing Event (Critical: {}) ---", event.is_critical);
    
    // Use match to handle