#!/usr/bin/env python3
import argparse
import asyncio
import subprocess

HOST_COLOR = "\033[94m"
CLIENT_COLOR = "\033[93m"
ENDC = "\033[0m"

parser = argparse.ArgumentParser()
executable_path = "./target/debug/netcanv"

def log(who: str, color: str, line: str):
    print(f"{color}{who.ljust(8)}{ENDC} {line}", end="")

async def run_client(room_id: str):
    cmd = [executable_path, "join-room", "-r", room_id]
    client = await asyncio.create_subprocess_exec(*cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if client.stderr is not None:
        while True:
            line = (await client.stderr.readline()).decode()
            if not line: # break loop when closed
                break
            log("CLIENT", CLIENT_COLOR, line)

async def run_host():
    tasks = []
    cmd = [executable_path, "host-room"]
    host = await asyncio.create_subprocess_exec(*cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    if host.stderr is not None:
        while True:
            line = (await host.stderr.readline()).decode()
            if not line: # break loop when closed
                break
            log("HOST", HOST_COLOR, line)
            # Find room ID
            if line.find("got free room ID") != -1:
                id = line.split("r:", 1)[1].strip() # room ID is after "r:"
                tasks.append(asyncio.create_task(run_client(id)))
    else:
        print("stderr is None")

    # Wait for all clients to finish
    await asyncio.gather(*tasks)

if __name__ == "__main__":
    args, rest = parser.parse_known_args()
    if len(rest) > 0 and rest[0] == "--": # remove "--"
        rest = rest[1:]
    if "--release" in rest:
        executable_path = "./target/release/netcanv"
    subprocess.run(["cargo", "build", *rest]) # build executable
    asyncio.run(run_host())
