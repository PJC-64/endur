if vim.g.loaded_endur == 1 then
  return
end
vim.g.loaded_endur = 1

-- Register user commands
vim.api.nvim_create_user_command("EndurWatch", function(opts)
  local path = opts.args ~= "" and vim.fn.fnamemodify(opts.args, ":p") or nil
  require("endur").check_and_watch(path)
end, {
  nargs = "?",
  complete = "file",
})

vim.api.nvim_create_user_command("EndurSnapshots", function()
  require("endur").snapshots()
end, {})

vim.api.nvim_create_user_command("EndurStatus", function()
  require("endur").print_status()
end, {})

