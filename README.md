# home-assistant-provision

`home-assistant-provision` aims to provide a declaratively managed [Home Assistant](https://www.home-assistant.io/).

It can currently manage:

- the owner account
- areas
- config entries of integrations, through their config flows and options flows

## Getting started

1. Build it: `cargo build --release`
2. Write a declaration:

```json
{
  "owner": { "name": "Swarsel", "username": "swarsel", "language": "en" },
  "areas": { "office": { "name": "Office" } },
  "entries": {
    "mqtt": { "domain": "mqtt", "steps": [ { "broker": "127.0.0.1", "port": 1883 } ] }
  }
}
```

3. Run it against the instance:

```bash
./target/release/home-assistant-provision \
  --declaration <your declaration json> \
  --state <file where the tool keeps track of what it created> \
  --password-file <file containing the owner password>
```

By default, the tool connects to `http://127.0.0.1:8123`; use `--url <your home assistant instance>` to reach something else.

The output state file maps the keys of your declaration to the IDs Home Assistant assigned. It should be kept between runs to prevent unnecessary item recreations.

## Managing config flows

A config flow is a sequence of forms. `steps` holds the answers, one object per form, in the order Home Assistant asks for them. If the flow asks for a step you did not declare, the tool stops and tells you the step name and its fields, which is the quickest way to find out what an integration wants.

`options` works the same way for the options flow of an entry. It is only run again when the declared options change.

Flows that need user interaction cannot be managed (however, manually created entries will not be touched by this tool).

## Reading values from files

Any value in `steps` or `options` can be replaced by `{ "_file": "/path/to/file" }`. The tool then submits the content of that file (without trailing newlines) instead. This can, for example, keep secrets out of the declaration, which matters on NixOS.
