export const LoomSessionEnv = async () => ({
  "shell.env": async (input, output) => {
    if (input.sessionID) {
      output.env.OPENCODE_SESSION_ID = input.sessionID;
    }
  },
});
