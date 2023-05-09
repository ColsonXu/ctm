# Concurrent Task Manager (CTM)

Team members:

- Colson Xu
- Yiyang Liu
- Chengzi Cao

## Demo Link
https://youtu.be/_eAdfxgVTLc

## Summary Description

The objective of this project is to create a task manager capable of executing multiple Linux commands concurrently in the background. Users will be able to queue commands and review the outcomes of completed tasks. By facilitating simultaneous execution of multiple tasks without waiting for one command to finish before initiating another, the task manager will enhance user workflow and boost productivity.

This application is not designed for executing simple commands; rather, its purpose is to concurrently run multiple extensive-duration commands in a centralized location, while offering performance monitoring capabilities.

## Project Execution Summary
We are curious about how to write interactive terminal user interfaces (TUIs), just like the one in `htop`

![htop](img/htop.png)

This project provided us an opportunity to dive deep into the process of making such a TUI application. In the beginning, we thought the "meat and potato" of this project is concurrently running the command entered. While it did present considerable challenge, it was only because of its interaction with the UI components.

I don't believe there are parts of the code that's algorithmically challenging, except for perhaps the async version of the backend (mentioned later). However, we learned a lot by building an UI for this program. Such as how UI components are built, updated, and displayed, how the backend should interact with the frontend, etc.

## Work Done and Lesson Learned
The structure of the UI is as follows:

```
CTM
├─ Running
│  ├─ Menu
│  ├─ Commands
│  ├─ Detail
│  ├─ Command Output
│  ├─ Command Line
├─ Finished
├─ Stats
│  ├─ CPU Usage
│  ├─ CPU Usage Chart
│  ├─ Memory Usage
│  ├─ Memory Usage Chart
├─ Map
│  ├─ Map of Developer Hometowns
├─ Help
│  ├─ Help Message
```

The most interesting parts to implement is the `Command Line` and the `Stats` page.

### Command Input
The crate we used for UI is [tui-rs](https://github.com/fdehau/tui-rs). It is a basic terminal user interface library written in Rust. It provides a handful of UI components called "widgets" that we can use to build the UI. However, this crate does not handle user input at all, including basic text boxes which we need for command input.

To implement a text field using available tools, we use `KeyCode` from `crossterm` to handle key press events and use them to manipulate a `String` which is then displayed on screen using a simple text display widget from `tui-rs`.

In order to achieve the shell-like behavior of being able to scroll through previously entered commands, we store each of the user's input in a vector, which is written to disk on program exit and loaded from disk on program entry. By writing it to disk, the command history becomes persistent.

### `Stats` Page
The `tui-rs` crate provides several interesting widgets that we wanted to incorporate into this program in a meaningful way. Two of which are the `Gauge` (progress bar) widget and the `Chart` widget. They are perfect for monitoring system resources. We are using crate `systemstat` to display CPU and RAM usage. The CPU usage is an average over a certain period by using `thread::sleep(dur: Duration)`. The code is as follows.

```rust
thread::sleep(Duration::from_secs(1));
let cpu = cpu.done().unwrap();
```

Normally, this is fine. However, the UI is run in a loop. If the thread that read the CPU usage need to call `thread::sleep(dur: Duration)`, the UI will become unresponsive. To overcome this problem, the part that reads the CPU and RAM usage is extracted into an async function and is spawn at the beginning of the program. With each refresh of the UI, the program will use `try_receive()` to obtain the latest data without blocking. The data is updated at every refresh no matter which page the user is on. This way, we can avoid the problem of getting out dated data when the user goes into the `stats` page. This is the performance logger of the program.

![stats page](img/stats.png)

#### `Gauge`
The `gauge` widget is essentially a progress bar that can show any percentage as a bar. To do this, we only need to get the most recent value from the performance logger. This is fine for `gauge`, however, a simple mechanism like this will not suffice for the usage history charts.

#### `chart`
The `chart` widget takes in a vector of data points and plot them as either a scatter plot or a line chart. For the purpose of showing the usage trend, we are taking the history of CPU and memory usage and plotting two line charts. This requires us to keep track of a log of past data points instead of only one most recent value. However, as the performance logger adds a data point every 1 second (tunable), the log will soon become unnecessarily large. The chart only plots 40 data points by default (tunable), so there is no need to keep more than 40 entires in the vector. To achieve this, we implemented a custom wrapper for `VecDeque`.
```rust
pub fn push(&mut self, value: T) {
    if self.data.len() == self.capacity {
        self.data.pop_front();
    }
    self.data.push_back(value);
}
```
When the internal `VecDeque` is at capacity, further pushes will pop the first value in the deque. A `VecDeque` is used instead of `Vec` exactly because of the frequent pop from the front. Finally, we implemented an iterator for the data structure in order to be able to use `map()`. By using this data structure, we can efficiently keep track of the most recent 40 data points.


### Backend
The backend of this program refers to the worker threads responsible for running the commands concurrently. There are two proposed ideas as to how to implement this.

1. Use a async runtime to manage the worker threads, the program will remain multithreaded, but the async runtime will make sure running the commands will not block user input. Finally, use channels to communicate between the worker threads and the UI.
2. Use the worker threads as is. After a thread is done executing a command, it will put the result into a HashMap. When the UI want to access the result, it will simply read from the HashMap. If the locks are managed correctly, command execution should not block user input.

The team then split into two implementing the backend in the design of their choice. At this point, the program only has a basic UI and a single-threaded backend, and a lot of the UI components are not implemented yet. Because of this, the "async" team decided to simply branch off of the backend and ditch the UI completely just to test the functionality of the backend during development. Meanwhile, the "sync" team was developing the backend to work with the UI while adding more features to the UI.

This inevitably created a huge problem when we tired to merge the code. After the "async" team finished, their code was completely independent of the current codebase and making the two parts work together will take a huge amount of refactoring.

In hindsight, we should have created an API specification using traits so that when the backend is finished, the UI can simply call the predefined methods to execute, and get results. It will also make the code more modular as the two backends will then be interchangeable. This is most likely one of the most important lesson learned from this project for all of the team members.

## External Crates
```toml
[dependencies]
crossterm = "0.26.1"
tui = "0.19.0"
chrono = "0.4.24"
async-std = "1.12.0"
project-root = "0.2.2"
systemstat = "0.2.3"
```

## Code Structure
The code consist of three files - `main.rs`, `lib.rs`, and `perf.rs`.

- `main.rs` is the code for UI. It draws components on screen, handles user input, and communicates with the backend.
- `lib.rs` is the code for the backend. It is responsible for spawning the worker threads and continuously taking waiting commands off the queue and executing them before storing their outputs.
- `perf.rs` stores the data structure used by the performance logger.

We put in some effort into breaking up the code, especially `main.rs`, which is over 800 lines. However, the code for user interface is inherently monolithic with few reuseable parts. The `Finished` page shares similar layout and components with the `Running` page, so the code for these two pages are extracted into a function. We could possibly extract the code for user input and put that into a separate file. However, the input handler need to orchestrate multiple moving parts of the user interface, so separating it from `main.rs` is unlikely to provide more benefit than costs. At the current state, we believe the project is broken down into reasonable pieces.

## Rusty Examples
```rust
#[derive(Default)]
pub struct Tasks {
    queue: Arc<Mutex<VecDeque<(usize, String)>>>,
    currently_running: Arc<Mutex<HashMap<usize, Task>>>,
    finished: Arc<Mutex<HashMap<usize, Task>>>,
}
```
This excerpt shows two benefits of using Rust, one is the use of struct. Structs can be used to define a type, in this case, the three data structures needed to maintain a the state of the core program is aggregated into one single struct called `Tasks`. It holds the command in queue, commands that are currently running, and commands that are finished and ready to be displayed in the `Finished` page.

```rust
let cpu_data_points = cpu_hist
    .iter()
    .enumerate()
    .map(|(i, load)| (i as f64, *load as f64))
    .collect::<Vec<_>>();
```

This is how we create the dataset the charts need. We need to turn every element in the data structure into an `f64`, and add it to a tuple along with its index. Rust's iterator in combination with a simple closure allows us to easily achieve this.

The third rusty feature is the use of `#[derive(Default)]`. This is a unique feature in Rust, Which is used to generate a default constructor if all of the fields in a struct implements `Default`.

```rust
#[derive(Copy, Clone, Debug, PartialEq)]
enum MenuItem {
    Running,
    Finished,
    Stats,
    Map,
    Help,
}
```
This second example is a `enum` used to store different menu tabs. One phrase I heard someone say is

> If you design your types and structs well, Rust can make sure illegal states are un-representable.

I really like this saying, and it shows when using Rust to build complex software. I used to love Python for its flexibility. Whatever you want to do, Python will allow it. This seems to speed up development since writing python is virtually no different than writing pseudo code. However, this flexibility is often an excuse for writing poorly designed software. When I wrote a Wireshark clone using Python, random contents can appear at random places because there are absolutely nothing stopping me from putting anything anywhere. When the program gets large, it is hard to manage which part should get its content from where. Rust, in contrast, like most other strongly-typed programming languages, will make sure the data you handle is of the expected type. What makes Rust stand out from other strongly-typed languages is that the use of `struct`, `enum`, and `trait` can also make sure the program never goes into an illegal state, and you will never be able to perform operation on a value that cannot be performed on that value.

In this example, `MenuItem` enum provides me with a definition for the state of the user interface. Since all of the code that need to interact with the UI tabs do so through `MenuItem` (for example, switching between tabs), there is no way that those code will put the program into an illegal state. Again, we see the use of `#[derive()]` here. This is a really convenient feature that saves a lot of time implementing the features ourselves.

## Challenges in Using Rust
I personally don't feel like there are any challenges building this program that are caused by using Rust. If anything, Rust simplified the development process. One can certainly argue the same program written in a language like Python will be significantly shorter. However, what Python cannot provide is the ease of maintenance and the guarantee that the code will not have any unexpected behavior caused by type errors, which Python programs are prone to. I think the time it takes me to type out those "boilerplate" code is significantly shorter than the time it would take me trying to figure out what caused my Python program to crash.

## Testing
The program is tested manually to make sure each UI element is working as intended. Necessary functionalities are added to ensure usability. The command output can be assumed to be correct since it is handled by the standard library with minimal intervention. Although we did not have the time to set up any automated testing, one idea is to use automated testing to test if the worker thread executed every command and correctly stored their output. This part can be tested independently of the UI.

## Limitations
Because of the scope of this project, we have to make some simplifying assumptions as to the command that will be run in this program.
1. The program cannot know if two command will conflict with each other (I doubt there is a way of knowing). So the program assumes the user made sure the commands entered are compatible of running concurrently with each other.
2. The `tui-rs` crate does not have mouse input, although not important, it is a nice-to-have.
3. The program does not handle any command that asks for user input. This is a limitation of `std::process`. There might be a way to inherit stdin from the parent process, but since `tui-rs` also doesn't handle input, we decided to leave this hot mess for future works.

## Running
Please make sure the program is run using `cargo run -r` so that super long outputs does not slow down the program.

## Alternative Designs
As mentioned before, the backend has two ideas for implementation. Half of the team wants to use an async runtime to bridge the frontend and backend to avoid blocking the UI. The other believes that synchronous code will suffice. The two designs are both implemented. However, the async version was developed somewhat independently of the UI codebase, making it incompatible with the finished program. Another reason it is not used in the final version of the code because it is somewhat unnecessary to use an async run time if locks are managed correctly. The data structures used in the async version still need to be put in Mutexes, thus, adding an async runtime on top only adds complexity. Nevertheless, it is still a working solution with a basic UI. We have included it in the submission as a separate crate named `ctm-async`.

## Lessons Learned
- In the early phase of the development, it is important to have a good design first. Especially if different teams are developing different part of the program. Having a good interface/trait in place can make sure that the end product can work together as intended.
- Although writing Rust programs can seem intimidating and disorientating for newcomers. It is actually easier than Python for large projects once you get the basics down. When writing Python, I constantly feel like I have to keep multiple moving parts in my mind. I have to know which part is of which type, how I can modify it, and when I can modify it. One misstep and the entire program can crash with a helplessly simple error message. Rust compiler will not only take care of the types for me, it also gives the move comprehensive and actionable error message I have ever seen. Actionable is a strange word to use on an error message, but I certainly feel like the Rust compile is teaching me how to write safe program. More often than not, it will just tell me how to fix the problem.
- The Rust ecosystem is still far from complete. Especially in the UI domain. Neither TUI nor GUI has a really comprehensive crate. This could be a great area for personal projects.

# Async Version
As the development of this program was split into implementing two designs in the early stage, the async team has also done significant work and has a working program as opposed to an abandoned one. As such, they decided to answer some of the questions separately here.

![ctm_async structure](/docs/img/async_struct.png)
#### Code structure of the code (what are the main components, the module dependency structure). Why was the project modularized in this way?
The main.rs initializes the necessary resources and starts the main executor function along with the UI components mentioned earlier. Below are the main components in the main.rs:

- `my_executor`: The main function for managing tasks. It spawns the UI task and the accept_outputs task, which are responsible for handling user inputs and processing the outputs from worker threads respectively.

- `accept_outputs`: This function processes the outputs from the worker threads and updating the task list and task results. It spawns two sub-tasks, update_task_list and update_task_result.

- `update_task_list`: This function updates the task list with the task IDs and corresponding thread IDs for the worker threads.

- `update_task_result`: This function is responsible for updating the task results hashmap with the latest results from the worker threads.

The `lib.rs` file contains the implementation of the Tasks struct, the functions associated with it. The main components of `lib.rs` are:

- `Tasks` struct: It stores a command queue and a history of executed commands, both of which are thread-safe data structures wrapped in `Arc<Mutex<>>`.

- `thread_function`: This function is executed by worker threads. It constantly checks for new tasks in the command queue, processes them, and sends the results back to the main executor.

- `create_threads`: This function initializes worker threads by passing the necessary resources, like tasks, id_senders, output_senders, wrong_commands, and tx_wrong, to the thread_function.

- `parse_command`: This function takes a command string and parses it into a Command object.

- `run_command`: This function takes a task and executes the associated command. If the command is executed successfully, it sends the output to the main executor. If there is an error, it adds the task ID to the wrong_commands HashSet and sends the updated HashSet through the tx_wrong channel.


### Were any parts of the code particularly difficult to express using Rust? What are the challenges in refining and/or refactoring this code to be a better example of idiomatic Rust?
The mixing of asynchronous and synchronous concurrency implementation in the code is challenging. We have to decide when to block the process and when not to. For example, the detection of wrong commands is designed to be blocked as we need to prevent these commands from adding to the result hashmap and therefore leads to a None in the entry and cannot be distinguished from running commands. On the other hand, we apply async on I/O to save the cost of using extra threads. As async programming tends to be more efficient for I/O-bound tasks, while concurrency is more suitable for CPU-bound tasks. 

Asynchronous concurrency in Rust is typically handled using async/await, while synchronous concurrency often uses threads and synchronization primitives like mutexes and channels. They have different paradigms for managing shared state and executing tasks. 

### Describe any approaches attempted and then abandoned and the reasons why. What did you learn by undertaking this project?
We planned to use sync for the whole project until we found the infinite loop of addressing input will block other executions. This makes us turn to async to lower the cost of concurrency. 

The second example is about the wrong commands. We decided to ignore the deprecated commands as they seemed to be trivial. Later we found that without distinguishing wrong commands from other commands results in the blocking and mismatching display of outputs. 

Thus, we learned that it is critical to have a detailed design before writing code for a project like this. Neglecting the design phase can result in unnecessary rewriting and wasted effort, as unforeseen issues may arise during implementation. By thoroughly planning the design beforehand, we can avoid potential pitfalls and reduce code refactoring. 
