local M = {}

-- Status cache storing repository statuses
M.status_cache = {}

-- Default configuration
M.config = {
  -- Path to the endur executable
  endur_path = "endur",
  -- Enable automatic watch registration on buffer write
  auto_watch = true,
  -- Enable statusline / winbar integration
  statusline = true,
}

-- Setup default highlights if not set
local function setup_highlights()
  vim.api.nvim_set_hl(0, "EndurClean", { link = "DiagnosticOk", default = true })
  vim.api.nvim_set_hl(0, "EndurModified", { link = "DiagnosticWarn", default = true })
end

-- Setup the plugin with user config
function M.setup(opts)
  M.config = vim.tbl_deep_extend("force", M.config, opts or {})

  setup_highlights()

  -- Check and start daemon if needed
  M.check_and_start_daemon()

  -- Setup autocommands if auto_watch or statusline is enabled
  if M.config.auto_watch or M.config.statusline then
    M.setup_autocommands()
  end
end

-- Launch the Telescope snapshots picker
function M.snapshots()
  local ok, telescope = pcall(require, "telescope")
  if not ok then
    vim.notify("Telescope.nvim is required for the snapshots picker", vim.log.levels.ERROR)
    return
  end
  telescope.load_extension("endur")
  telescope.extensions.endur.snapshots()
end

-- Setup autocommands to run endur watch on active buffers
function M.setup_autocommands()
  local group = vim.api.nvim_create_augroup("Endur", { clear = true })

  -- Watch git repositories automatically when writing files
  if M.config.auto_watch then
    vim.api.nvim_create_autocmd("BufWritePost", {
      group = group,
      pattern = "*",
      callback = function()
        M.check_and_watch(nil, function()
          M.update_status()
        end)
      end,
    })
  end

  -- Update status when entering a buffer or focusing Vim
  if M.config.statusline then
    vim.api.nvim_create_autocmd({ "BufEnter", "FocusGained" }, {
      group = group,
      pattern = "*",
      callback = function()
        M.update_status()
      end,
    })
  end
end

-- Runs `endur watch` on the parent directory of the current buffer if inside a Git repo
function M.check_and_watch(path, callback)
  local endur_path = M.config.endur_path
  if vim.fn.executable(endur_path) == 0 then
    vim.notify("endur: executable '" .. endur_path .. "' not found or not executable.", vim.log.levels.ERROR)
    if callback then callback() end
    return
  end

  local file_path = path or vim.api.nvim_buf_get_name(0)
  if file_path == "" then
    if callback then callback() end
    return
  end

  local dir
  if vim.fn.isdirectory(file_path) == 1 then
    dir = file_path
  else
    dir = vim.fs.dirname(file_path)
  end

  -- Check if directory is in a Git repository
  local git_dir = vim.fs.find(".git", { path = dir, upward = true })[1]
  if not git_dir then
    if path then
      vim.notify("endur: '" .. file_path .. "' is not in a Git repository", vim.log.levels.ERROR)
    end
    if callback then callback() end
    return
  end

  local git_root = vim.fs.dirname(git_dir)
  git_root = vim.fn.resolve(git_root)

  -- Execute endur watch asynchronously
  local stdout = {}
  local stderr = {}
  local cmd = { endur_path, "watch", git_root }
  local job_id = vim.fn.jobstart(cmd, {
    stdout_buffered = true,
    stderr_buffered = true,
    on_stdout = function(_, data)
      if data then
        for _, line in ipairs(data) do
          if line ~= "" then table.insert(stdout, line) end
        end
      end
    end,
    on_stderr = function(_, data)
      if data then
        for _, line in ipairs(data) do
          if line ~= "" then table.insert(stderr, line) end
        end
      end
    end,
    on_exit = function(_, exit_code)
      if exit_code == 0 then
        if path then
          vim.notify("endur: now watching repository at '" .. git_root .. "'", vim.log.levels.INFO)
        end
      else
        local err_msg = table.concat(stderr, "\n")
        if err_msg == "" then err_msg = table.concat(stdout, "\n") end
        
        if err_msg:match("already being watched") then
          if path then
            vim.notify("endur: repository at '" .. git_root .. "' is already being watched.", vim.log.levels.INFO)
          end
        else
          vim.notify("endur: watch command failed for '" .. git_root .. "' (exit code: " .. exit_code .. "): " .. err_msg, vim.log.levels.ERROR)
        end
      end
      if callback then callback() end
    end,
  })

  if job_id <= 0 then
    vim.notify("endur: failed to start watch process. Check configuration.", vim.log.levels.ERROR)
    if callback then callback() end
  end
end

-- Checks if endur is running. If not, starts it.
function M.check_and_start_daemon()
  local endur_path = M.config.endur_path
  if vim.fn.executable(endur_path) == 0 then
    vim.notify("endur: executable '" .. endur_path .. "' not found in PATH.", vim.log.levels.ERROR)
    return
  end

  local info_cmd = { endur_path, "info" }
  local stdout = {}
  local job_id = vim.fn.jobstart(info_cmd, {
    stdout_buffered = true,
    on_stdout = function(_, data)
      if data then
        for _, line in ipairs(data) do
          table.insert(stdout, line)
        end
      end
    end,
    on_exit = function(_, exit_code)
      if exit_code ~= 0 then
        if exit_code ~= 143 then
          local err_msg = table.concat(stdout, "\n")
          vim.notify("endur: info command failed with exit status " .. exit_code .. ": " .. err_msg, vim.log.levels.ERROR)
        end
        return
      end

      local is_running = false
      for _, line in ipairs(stdout) do
        if line:match("Server:%s*Running") then
          is_running = true
          break
        end
      end

      if not is_running then
        local serve_cmd = { endur_path, "serve" }
        local serve_job = vim.fn.jobstart(serve_cmd, {
          detach = true,
          on_exit = function(_, serve_exit)
            if serve_exit ~= 0 and serve_exit ~= 143 then
              vim.notify("endur: failed to start background daemon (exit code: " .. serve_exit .. ")", vim.log.levels.ERROR)
            end
          end
        })
        if serve_job <= 0 then
          vim.notify("endur: failed to spawn serve command.", vim.log.levels.ERROR)
        else
          vim.defer_fn(function()
            M.update_status()
          end, 1000)
        end
      else
        -- Extract status cache from info output
        local new_cache = {}
        for _, line in ipairs(stdout) do
          local status_code, raw_path, details = line:match("^%[(.-)%]([^:]+):%s*(.*)")
          if status_code and raw_path then
            local repo_path = vim.fn.resolve(vim.trim(raw_path))
            new_cache[repo_path] = {
              status = status_code,
              details = details
            }
          end
        end
        M.status_cache = new_cache
        vim.cmd("redrawstatus")
      end
    end
  })

  if job_id <= 0 then
    vim.notify("endur: failed to execute info command.", vim.log.levels.ERROR)
  end
end

-- Refresh status cache from endur info
function M.update_status(callback)
  local endur_path = M.config.endur_path
  if vim.fn.executable(endur_path) == 0 then
    if callback then callback() end
    return
  end

  local info_cmd = { endur_path, "info" }
  local stdout = {}
  local job_id = vim.fn.jobstart(info_cmd, {
    stdout_buffered = true,
    on_stdout = function(_, data)
      if data then
        for _, line in ipairs(data) do
          table.insert(stdout, line)
        end
      end
    end,
    on_exit = function(_, exit_code)
      if exit_code == 0 then
        local new_cache = {}
        for _, line in ipairs(stdout) do
          local status_code, raw_path, details = line:match("^%[(.-)%]([^:]+):%s*(.*)")
          if status_code and raw_path then
            local repo_path = vim.fn.resolve(vim.trim(raw_path))
            new_cache[repo_path] = {
              status = status_code,
              details = details
            }
          end
        end
        M.status_cache = new_cache
        vim.cmd("redrawstatus")
      end
      if callback then callback() end
    end
  })

  if job_id <= 0 and callback then
    callback()
  end
end

-- Resolves the current buffer's git root and gets its status
function M.get_current_repo_status()
  local file_path = vim.api.nvim_buf_get_name(0)
  if file_path == "" then return nil end

  local dir = vim.fs.dirname(file_path)
  local git_dir = vim.fs.find(".git", { path = dir, upward = true })[1]
  if not git_dir then return nil end

  local git_root = vim.fs.dirname(git_dir)
  git_root = vim.fn.resolve(git_root)
  
  return M.status_cache[git_root]
end

-- A beautiful statusline component
function M.statusline()
  if not M.config.statusline then return "" end
  local status_info = M.get_current_repo_status()
  if not status_info then return "" end

  local icon, hl, text
  if status_info.status == "OK" then
    icon = "󰄬"
    hl = "%#EndurClean#"
    text = "Clean"
  elseif status_info.status == "M" then
    icon = "󰏬"
    hl = "%#EndurModified#"
    text = "Modified"
  else
    icon = "󰁯"
    hl = "%#EndurModified#"
    text = status_info.status
  end

  local backups = status_info.details:match("(%d+) backups")
  if backups then
    text = text .. " (" .. backups .. ")"
  end

  return string.format("%s %s Endur %s %%*", hl, icon, text)
end

-- Winbar integration (same format as statusline, but can be customized)
function M.winbar()
  return M.statusline()
end

-- A raw statusline component string without highlight codes (useful for lualine, etc.)
function M.statusline_raw()
  local status_info = M.get_current_repo_status()
  if not status_info then return "" end

  local icon, text
  if status_info.status == "OK" then
    icon = "󰄬"
    text = "Clean"
  elseif status_info.status == "M" then
    icon = "󰏬"
    text = "Modified"
  else
    icon = "󰁯"
    text = status_info.status
  end

  local backups = status_info.details:match("(%d+) backups")
  if backups then
    text = text .. " (" .. backups .. ")"
  end

  return string.format("%s Endur %s", icon, text)
end

return M
