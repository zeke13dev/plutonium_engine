# ECS and Input

## ECS
- Entities (u32), components stored by type name.
- Resources keyed by type name; Time and FrameNumber provided.
- Schedules: startup, fixed_update, update, render.
- Events: send_event/drain_events per type.

## Input
- InputState: pressed/just_pressed/just_released sets; mouse position and LMB edges.
- ActionMap: binding strings to actions; action_just_pressed.
- Demo: updates InputState from FrameContext each frame.
