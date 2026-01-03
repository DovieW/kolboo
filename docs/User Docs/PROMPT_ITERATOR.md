# Prompt Iterator

The Prompt Iterator is a tool that helps you improve your AI prompts by learning from examples of what you want.

## What is it?

When you're using Kolboo's AI rewrite feature, sometimes the output isn't quite what you want. The Prompt Iterator helps you fix this by analyzing examples of incorrect output and generating an improved prompt that produces better results.

## How to use it

1. **Open the Prompt Iterator**
   - Go to Settings → Prompts
   - Scroll down to the "Test rewrite" section
   - Below it, you'll find the "Prompt Iterator" accordion item
   - Click "Open Prompt Iterator"

2. **Provide your examples**
   - **Input**: The raw transcript you gave to the AI
   - **Actual Output**: The incorrect or undesired output you received
   - **Desired Output**: What you actually wanted the AI to produce
   - **Reasoning** (optional): Explain why the actual output was wrong

3. **Generate an improved prompt**
   - Click "Generate Improved Prompt"
   - The AI will analyze the difference between actual and desired output
   - It will generate a new prompt designed to produce the desired result

4. **Test and apply**
   - Review the improved prompt
   - Click "Apply to Settings" to use it
   - Test it using the "Test rewrite" section above to verify it works

## Tips for best results

- **Be specific**: Provide clear, concrete examples of what you want
- **Use real data**: Use actual transcripts and outputs you've encountered
- **Explain your reasoning**: The optional reasoning field helps the AI understand what you care about
- **Iterate**: You can run this multiple times with different examples to refine your prompt further
- **Test thoroughly**: Always test the improved prompt before relying on it

## Example workflow

Let's say you have a prompt that's supposed to remove filler words, but it's also removing important words:

1. **Input**: "um so I think we should like definitely go to the meeting"
2. **Actual Output**: "I think we should go to the meeting" (removed "definitely")
3. **Desired Output**: "I think we should definitely go to the meeting"
4. **Reasoning**: "The word 'definitely' is not a filler word and should be preserved"

The Prompt Iterator will then generate an improved prompt that distinguishes between real filler words (um, like) and emphatic words (definitely).

## Technical details

- The feature uses your configured LLM provider and model
- It respects your profile settings (if you're using per-program profiles)
- The improved prompts are applied to the "Core Formatting Rules" section
- Cost is tracked like any other LLM usage
