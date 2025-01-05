# evocaition

## Overview

evocaition is a command-line tool designed to interact with AI large language models (LLMs) via APIs. It provides a straightforward interface to send prompts and receive completions from various AI models, making it easy for users to engage with AI technologies directly from the terminal.

It is designed to be a tool that plays well with other command-line tools. It can ingest the prompt from *stdin* and then write it's generated output to *stdout*. This also means that it's stateless and does not do multi-turn conversations. One prompt in ... one response out.

## Features

- **Endpoint Customization:** Defaults to using [openrouter](https://openrouter.ai/) but has been tested to work against [LM Studio's server](https://lmstudio.ai/docs/api/server) as well.
- **Prompt Interaction:** Users can provide prompts either through command-line arguments or by reading from standard input.
- **Model Customization:** Allows selection of different models and customization of generation parameters such as maximum tokens, temperature, top-p, etc.
- **Streaming Output:** Option to stream the response as it is being generated, useful for real-time interactions.
- **Image Support:** Ability to attach images to user requests, enhancing the interactivity for models that support multi-modal inputs.
- **VS Code Support:** Once installed locally, the [Evocaition VS Code Extension](https://marketplace.visualstudio.com/items?itemName=tbogdala.evocaition-vscext) can be installed to generate text within the editor buffers.

## Usage

### Basic Usage

To use evocaition, you need to have an API key for the service you are interacting with. This can be provided via the `--key` flag or by setting the `OPENROUTER_API_KEY` environment variable. This is not required to be set if you're connecting to an endpoint that doesn't do authentication, such as LM Studio's server.

Sample basic use:

```sh
evocaition --prompt "What is the meaning of life?"
```

The received response (cut off at 100 tokens):

```
Ah, the age-old question! It's a question that has plagued philosophers, 
theologians, and everyday people for centuries. The truth is, there isn't
one universally accepted answer. What the "meaning of life" is is deeply 
personal and subjective.

Here's a breakdown of why it's so complex and some common perspectives:

**Why There's No Single Answer:**

* **Subjectivity:** Meaning is often derived from individual experiences, 
values, and beliefs.
```

### Specifying Models

When using openrouter, supply the model want to use like this:

```sh
evocaition --prompt "Please write a clever haiku." \
    --model-id "meta-llama/llama-3.2-1b-instruct"
```

If using another endpoint like LM Studio, you may need to specify it differently:

```sh
evocaition --prompt "Please write a clever haiku." \
    --model-id "llama-3.2-1b-instruct" \
    --api http://127.0.0.1:1234
```

### Sampler Parameters

Multiple sampler parameters can be set as well:

```sh
evocaition --prompt "Make a bold prediction for the year 2028." \
    --model-id "meta-llama/llama-3.2-1b-instruct" \
    -n 512 --temp 1.8 --top-p 0.8 --min-p 0.05 --top-k 80 --rep-pen 1.04
```

### Streaming Responses

If you wish to see the output as it is received, enable streaming with the
`-s` parameter and the result will be written to *stdout* as it arrives.

```sh
evocaition --prompt "Make a bold prediction for the year 2028." -n 512 -s
```

### Chat or Plain Completion

By default, evocaition uses the 'chat' endpoints where the prompt is placed
inside a user message behind the scenes when sent to the API endpoint
(though without any modification other than to fit the JSON structure of
the POST request). 

It is possible to use the 'legacy' completion API instead:

```sh
evocaition --prompt "The following message is " --plain -n 100 \
    --model-id "mistralai/mistral-nemo"
```

The output received:

```
13 words long. The second part is run together:

Pyramids are 10,000 years old. Egypt was the richest for 1000 years.

The second message in this case is "EGYPTISTHEVAST."

A message where all spaces are replaced with letters is _much_ easier 
to solve for a properly trained codebreaker. If the words are short 
and well known, it's possible to solve this scheme purely by trial and error
```

### Image Support

If using a multimodal model, you can specify a local file path to have an image on
your machine uploaded to the API endpoint with the prompt. The supported file types
are JPEG, PNG and WEBP.

```sh
evocaition --prompt "Describe this image with some gusto." \
    --model-id "meta-llama/llama-3.2-11b-vision-instruct" \
    --image ~/Pictures/Profile.png \
    -n 256
```

The output received:

```
The image showcases an artful and humorous canine astronaut in a vibrant 
portrait, set against an American flag backdrop.

The canine, depicted and wearing a helmet and a green laboratory jumpsuit with 
an American flag patch, assumes a serious stance and looks straight into the 
viewer, communicating a stoic determination. The dog's head is accentuated 
with dark hair framing its light-colored face, featuring a brown nose with 
distinctive pink spots on its snout, and large fluffy ears to complement the 
verall canine aesthetic. Its exceptional bionic eyes hold a steely gaze, 
exuding a steadfast mission to space adventure and exploration.  Notable 
features of the astronaut suit include a large, round, blue ring neck flap, 
fanny pack, black shoulder straps, brass or silver buckles, and metal-encased 
dials and gauges attached to the identical chest and the forearms.

The background of the image is lively, with blurred but distinguishable color 
patches of the American flag and a consistent dark, uniform and sleek, black 
background.
```

Additionally, you can specify a URL for an image instead of a local file:

```sh
evocaition --prompt "Describe this image with some gusto." \
    --model-id "google/gemini-flash-1.5-8b" \
    --image "https://upload.wikimedia.org/wikipedia/commons/d/d2/LowerFallsJackson1871.jpg" \
    -n 256
```

The output received:

```
A breathtaking black and white, likely vintage photograph, showcasing the 
grandeur of the Great Falls, likely in Yellowstone National Park.


The image is dramatic, angled down towards the falls, revealing the sheer cliff 
faces and the powerful cascade of water plunging downwards.  The waterfall 
itself is a spectacular, misty, white ribbon that streaks across the frame, 
signifying its powerful force and the volume of water.


The surrounding landscape is a mix of rugged, light gray rock formations, and 
dense evergreen forests clinging to the slopes.   The trees are sharply defined 
against the sky, adding depth and texture to the overall composition. Small, 
scattered patches of what looks like snow or ice cling to the steep, shadowed 
slopes.  The photo conveys a sense of vastness and awe-inspiring natural beauty.
The quality suggests a pioneering, almost documentary, style of photography, 
perhaps from the late 19th or early 20th century, capturing a moment in time. 
The photographer has skillfully used light and shadow to highlight the textures 
and depths of the landscape. The water flowing around the base of the falls is 
also a strong element in the scene, giving a kinetic sense to the image.
```


## Build and Install

Building from source requires a [Rust toolchain](https://rustup.rs/):

```bash
git clone https://github.com/tbogdala/evocaition.git
cd evocaition
cargo build --release

# want to put the executable in your path?
cargo install --path .
```


## Suggestions and Future Plans

Please, please, please ... if you have an idea for this tool that you want,
please drop an issue or start a discussion thread.

A future version will probably integrate llama.cpp support through my
[woolyrust](https://github.com/tbogdala/woolyrust) Rust bindings. Due to 
compile time and deployment impacts, this will likely be gated behind
a cargo feature. 


## License

evocaition is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
