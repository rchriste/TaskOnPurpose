# _On Purpose: Neurodiverse Time Mastery_
## Why does this project exist?

I believe that software can assist with mental health by being designed to better help us with time management. To better make my point I want to bring up being blind, because when software is designed to work well for the blind, all depending on how well it is implemented the software can change from being a barrier to an indispensable tool. I bring this up because I believe we have an opportunity to assist the neurodiverse with time management by making changes to software, but similar to software features for the blind doing this properly requires changes across the full software ecosystem. **The key is to make improvements in areas that have clinical impact and it is my understanding and experience that in a clinical setting, interactions with others and time management are the two big topics and how software is designed is very tied into the time management side of the problem. A certain thing about software might be a slight annoyance for many but for someone who is neurodiverse it is more than annoying rather it inhibits their ability to function and get things done. I believe a focus on making software work better for personal time management can help the neurodiverse and have a positive impact on society.**

This project exists because I want to be part of the solution. My goal is to implement something very practical starting with the feature of deciding what to do and eventually expanding from that point. I, Russ, am the first customer and this project is being implemented in the Rust programming language. This is currently an unpaid hobby project for me and my limited resources and time have a deep impact on this project. Because of this my current goals and motivations are much more about spreading these ideas and advocating for them than it is to become a viable open source project. However I believe becoming an open source project with active users and Github stars will help the ideas spread and will help these ideas get noticed and picked up by existing software so I am looking to grow my user base and take on contributors that also believe in the vision. Once things get further along I intend on releasing binaries, until then if you want to try it you will need to either wait and check back or build it from source.

## What expectations we should have for time management software:

* Help pick which item to do right now.
* Help transition from one task to the next.
* Help save, remember, and resume work.
* Help avoid distractions, stay on task, & remember the purpose behind the work.
* Help avoid surprises and be aware of and prepared for what is upcoming.
* Help with work-life balance and balancing the different areas of my life with each other in general.
* Help recall and summarize what was done.
* Help reflect, learn from, and celebrate both my effort and my accomplishments.

[You can watch a YouTube video I made that explains these ideas further here.](https://youtu.be/Gji6Yf8byts)

## Existing solutions

The problem that I have with existing self-help time management solutions is almost no one seems to stick with these systems over the course of many years. Especially when someone faces a crises these other system are more commonly abandoned rather than leaned into and relied on. Another signal that the ideal solution doesn't yet exist is the fact that these existing time management self-help systems might be more convenient to carry out with software, but software is not strictly required. Imagine using a modern word processor for a couple years and then returning to a typewriter; and yet people return to a simple bullet list all the time after trying out the time management software of today.

## What is _On Purpose_?

All to often I have heard or read wonderful guidance on how to manage my time, but the guidance is very principal based with implementation details left to the reader. _On Purpose_ is meant to fill this void.

I envision _On Purpose_ as the personal or individualized view. When it comes to groups or companies there are many existing systems and programs that help plan, implement, and track work. _On Purpose_ on the other hand is for the individual. It aspires to integrate with these existing programs and help that person track and balance all that they need to keep track of. This includes inside and outside of your career or schooling, meaning the ability to understand your regular routine and what is upcoming. For employment it includes the core work that you are getting paid to do and the non-core work that for various reasons is still worth doing.

Rather than being designed to help the group track your work it is designed for the individual doing the work so it is next step driven. _On Purpose_ encourages you to break work down to next steps until you get to a next step that you can do. When the next step is to wait for someone to get back to you or wait for some program or process to do something then _On Purpose_ is meant to help save or remember what is required to easily resume the work later and it is designed to integrate with existing systems to automate knowing when you can return to something. _On Purpose_ is intended to do a lot of this automatically but if it can't then you set a timer for when to check back.

_On Purpose_ is meant to be a program that integrates with existing systems rather than replaces them. My goal with _On Purpose_ is to integrate with the Microsoft Platform because I am a long time Microsoft employee and I use Microsoft products in my day job and at home. This means that I plan on having _On Purpose_ integrate with the following products:
* Outlook Email, Calendaring, & To Do
* Microsoft OneNote
* Microsoft Teams
* Azure Dev Ops
* OneDrive

## My Journey

I am a software programmer and a few years ago I picked up time management as my main hobby. I initially wrote a sidecar application, in C#, for an existing to do app that I used at the time. This experience convinced me that I needed to fully control the UI. I then tried designing a UI, but none of my ideas were very good so in search of better ideas I brainstormed and came up with a thirty page design document of what experiences I wanted when using a PC or phone. I still lacked a UI and I was foggy on the details but I decided to learn Rust and try implementing a text based prototype with the goal of figuring out a UI, but after many rewrites and fights with the Rust borrow checker I ultimately abandoned this project and decided to focus my energy on creating a very detail oriented presentation, well over a hundred slides, not to show others but rather to work out for myself UI mock-ups and underlying reasons for the various areas and parts of the program. I also tried out and refined some of my UI ideas in Visio and OneNote. I both believe deeply in these ideas and I also believe that if I am ever given the opportunity to fully realize this vision a lot of further refinement will be needed.

My next goal was to create a program that I myself use day-in and day-out as my personal to do application with the core feature being to help me decide what to do next. I created this GitHub project with that initial goal in mind and I have been making progress. 
I am proud to say that I have been benefiting from _On Purpose_ constantly and every day since January of 2024. Near the beginning of summer I came up with an idea for how to better determine what to do by leveraging relative importance, task urgency and in the moment priorities. The beginnings of that idea is now implemented and I am starting to feel like I am gathering the evidence necessary to personally feel good about these ideas. I am now looking towards starting to share these ideas more broadly and having a program that people can try that is usable enough to release binaries for.

## Core Rust Crates of _On Purpose_

Currently _On Purpose_ is a text based Windows program written in Rust. It brings up a selection of items using the [inquire](https://github.com/mikaelmello/inquire) crate. This is the current UI because it is the easiest to experiment with as I work out the core feature set. In time I intend to adopt [Ratui](https://ratatui.rs/) for a more fully featured but still text based UI. 

I am also paying attention to GUI app development in Rust. I am doing this for two reasons, on the desktop I would like to eventually be an always viewable docked application similar to the Windows start bar. I also hope to eventually create an Android app for the phone and investigate the idea of integrating in some fashion with the Android operating system. I am paying attention to the following projects and I intend to eventually try to prototype _On Purpose_ in each of them: [Makepad](https://github.com/makepad/makepad), [Dioxus](https://dioxuslabs.com/), [Iced](https://iced.rs/) and [Xilem](https://github.com/linebender/xilem). Also to better share code between platforms I am paying attention to [Robius](https://robius.rs/).

The data storage layer is implemented as an embedded [SurrealDB](https://github.com/surrealdb/surrealdb) database. You can think of this like [SQLLite](https://www.sqlite.org/index.html) except I am using [SurrealDB](https://github.com/surrealdb/surrealdb). Currently I only save data locally. Sync'ing the data between machines is planned, however I want to avoid having a service for multiple reasons. Ideally I would sync the data between machines using the [Microsoft Graph To Do REST APIs](https://lib.rs/crates/graph-rs-sdk), but I doubt I can make my to do items compatible with the Microsoft To Do schema. I plan on trying to extend the To Do API with a json blob in the To Do Notes section, but I'm not sure how much I should be doing this. Also there are things to sync beyond to do items, like time spent logs so beyond the To Do REST API I plan on also syncing data between machines by placing files in [OneDrive](https://lib.rs/crates/onedrive).

## Installing _On Purpose_

If you want to try it now you will need to compile it and use the Rust tool `Cargo install` to install it. As of today I expect _On Purpose_ to work on both Windows and Linux but I expect the Windows side to eventually be more fully featured as I do have plans to integrate with various Windows API in time. I will mention that setting up the Surreal DB build dependency is more of a pain in Windows proper than the convenient steps you can follow inside Windows' Linux WSL layer. But Windows is what I am currently using.

### Compiling _On Purpose_

Compiling On Purpose requires the Rust toolchain and it requires installing various things as well so the Surreal DB dependencies can compile. These other things are things like LLVM and some GNU tools. This is required because I use [Surreal DB](https://github.com/surrealdb/surrealdb) as an embedded database that persists data to disk.

* [Install Rust from here](https://rustup.rs)
* [Instructions for how to install the SurrealDB dependencies are here](https://github.com/surrealdb/surrealdb/blob/main/doc/BUILDING.md)

If you want to be able to just type `on_purpose` from a console window then you can install _On Purpose_ by doing `cargo install --path console` then as further changes are checked in you can do a `git pull` and rerun the cargo install command to update to the latest version.

Because it takes a while to build I will generally use the older version of _On Purpose_ while the new one compiles and then after I get an error that the file is in use I will close the _On Purpose_ program and rerun the cargo install command a second time to install the updated binary.

### Using On Purpose with Windows Terminal

In order for the Emoji and Unicode char to display properly you need to enable the new "Atlas" rendering engine. Go to Settings -> Rendering -> Engine and turn on `Use the new Text Render ("AtlasEngine")`

### Entering dates and times (quick tip)

When the app prompts for a date/time you can type relative phrases. Examples:
- `Tomorrow 3pm` schedules for tomorrow at 3pm.
- `Next day 3pm` works the same as `Tomorrow 3pm` (new alias).
- Short times like `9am` assume today; full dates like `1/15/2025 4pm` also work.