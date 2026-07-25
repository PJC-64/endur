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

-- Helper to get and parse snapshot items
local function trim(s)
  return (s:gsub("^%s*(.-)%s*$", "%1"))
end

function M.get_snapshots_items(git_root)
  local endur_path = M.config.endur_path
  local command = { endur_path, "list-snapshots", git_root }
  local output = vim.fn.systemlist(command)
  if vim.v.shell_error ~= 0 then
    local err = table.concat(output, "\n")
    vim.notify("Failed to fetch Endur snapshots: " .. err, vim.log.levels.ERROR)
    return {}
  end

  local items = {}
  local hashes = {}
  for i = 3, #output do
    local line = output[i]
    if line ~= "" then
      local hash = trim(line:sub(1, 40))
      if hash:match("^%x+$") and #hash == 40 then
        local datetime = trim(line:sub(42, 66))
        local changes = trim(line:sub(68))
        local item = {
          hash = hash,
          datetime = datetime,
          changes = changes,
          display = string.format("%s │ %s │ %s", hash:sub(1, 8), datetime, changes),
          files_str = changes,
        }
        table.insert(items, item)
        table.insert(hashes, hash)
      end
    end
  end

  if #hashes > 0 then
    local show_cmd = { "git", "-C", git_root, "show", "--name-only", "--format=HASH:%H" }
    vim.list_extend(show_cmd, hashes)
    local show_out = vim.fn.systemlist(show_cmd)
    if vim.v.shell_error == 0 then
      local current_hash = nil
      local hash_files = {}
      for _, line in ipairs(show_out) do
        if line ~= "" then
          local matched_hash = line:match("^HASH:(%x+)")
          if matched_hash then
            current_hash = matched_hash
            hash_files[current_hash] = {}
          elseif current_hash then
            local filename = trim(line)
            if filename ~= "" then
              table.insert(hash_files[current_hash], filename)
            end
          end
        end
      end

      for _, item in ipairs(items) do
        local files = hash_files[item.hash] or {}
        local files_str
        if #files > 3 then
          files_str = string.format("%s, %s, and %d more", files[1], files[2], #files - 2)
        elseif #files > 0 then
          files_str = table.concat(files, ", ")
        else
          files_str = item.changes
        end
        item.files_str = files_str
        item.display = string.format("%s │ %s │ %s", item.hash:sub(1, 8), item.datetime, files_str)
      end
    end
  end

  return items
end

-- Launch the snapshots picker (automatically chooses Snacks, Fzf-Lua, or Telescope)
function M.snapshots()
  local has_snacks, snacks = pcall(require, "snacks")
  if has_snacks and snacks.picker then
    M.snapshots_snacks()
    return
  end

  local has_fzf, fzf = pcall(require, "fzf-lua")
  if has_fzf then
    M.snapshots_fzf()
    return
  end

  local ok, telescope = pcall(require, "telescope")
  if ok then
    telescope.load_extension("endur")
    telescope.extensions.endur.snapshots()
    return
  end

  vim.notify("endur: No supported picker found. Please install snacks.nvim, fzf-lua, or telescope.nvim.", vim.log.levels.ERROR)
end

-- Snacks Picker Implementation
function M.snapshots_snacks()
  local current_dir = vim.fn.expand("%:p:h")
  local current_file = vim.fn.expand("%:p:t")
  local current_file_path = vim.api.nvim_buf_get_name(0)

  local git_dir = vim.fs.find(".git", { path = current_dir, upward = true })[1]
  if not git_dir then
    vim.notify("endur: Not in a Git repository", vim.log.levels.ERROR)
    return
  end
  local git_root = vim.fs.dirname(git_dir)

  local items = M.get_snapshots_items(git_root)
  if #items == 0 then return end

  local snacks_items = {}
  for _, item in ipairs(items) do
    table.insert(snacks_items, {
      text = item.display,
      hash = item.hash,
      datetime = item.datetime,
      changes = item.changes,
      files_str = item.files_str,
    })
  end

  local snacks = require("snacks")
  local endur_path = M.config.endur_path

  snacks.picker.pick({
    source = "endur",
    prompt = "Endur Snapshots (" .. vim.fs.basename(git_root) .. ")",
    items = snacks_items,
    preview = function(ctx)
      if not ctx or not ctx.item then return end
      local cmd = { "git", "-C", git_root, "show", "--stat", "-p", ctx.item.hash }
      vim.fn.jobstart(cmd, {
        stdout_buffered = true,
        on_stdout = function(_, data)
          if data and vim.api.nvim_buf_is_valid(ctx.buf) then
            vim.api.nvim_buf_set_lines(ctx.buf, 0, -1, false, data)
            vim.api.nvim_set_option_value("filetype", "diff", { buf = ctx.buf })
          end
        end
      })
    end,
    confirm = function(picker, item)
      picker:close()
      if not item then return end
      local hash = item.hash
      vim.ui.input({
        prompt = "Restore ENTIRE repository to snapshot " .. hash:sub(1, 8) .. "? Type 'yes' to confirm: "
      }, function(input)
        if input and input:lower() == "yes" then
          vim.notify("Restoring repository to snapshot " .. hash:sub(1, 8) .. "...")
          local out = vim.fn.system({ endur_path, "restore", hash, git_root })
          if vim.v.shell_error ~= 0 then
            vim.notify("endur: restore failed: " .. out, vim.log.levels.ERROR)
          else
            vim.notify("endur: successfully restored repository to snapshot " .. hash:sub(1, 8), vim.log.levels.INFO)
            vim.cmd("edit!")
          end
        else
          vim.notify("endur: restore aborted", vim.log.levels.INFO)
        end
      end)
    end,
    win = {
      input = {
        keys = {
          ["<C-f>"] = { "restore_file", mode = { "i", "n" } },
        },
      },
    },
    actions = {
      restore_file = function(picker, item)
        picker:close()
        item = item or picker:current()
        if not item then return end
        local hash = item.hash
        vim.ui.input({
          prompt = "Restore current file '" .. current_file .. "' to snapshot " .. hash:sub(1, 8) .. "? (y/N): "
        }, function(input)
          if input and input:lower():match("^y") then
            vim.notify("Restoring " .. current_file .. " to snapshot " .. hash:sub(1, 8) .. "...")
            local out = vim.fn.system({ endur_path, "restore", hash, git_root, "-f", current_file_path })
            if vim.v.shell_error ~= 0 then
              vim.notify("endur: restore failed: " .. out, vim.log.levels.ERROR)
            else
              vim.notify("endur: successfully restored " .. current_file .. " to snapshot " .. hash:sub(1, 8), vim.log.levels.INFO)
              vim.cmd("edit!")
            end
          else
            vim.notify("endur: restore aborted", vim.log.levels.INFO)
          end
        end)
      end
    }
  })
end

-- Fzf-Lua Picker Implementation
function M.snapshots_fzf()
  local current_dir = vim.fn.expand("%:p:h")
  local current_file = vim.fn.expand("%:p:t")
  local current_file_path = vim.api.nvim_buf_get_name(0)

  local git_dir = vim.fs.find(".git", { path = current_dir, upward = true })[1]
  if not git_dir then
    vim.notify("endur: Not in a Git repository", vim.log.levels.ERROR)
    return
  end
  local git_root = vim.fs.dirname(git_dir)

  local items = M.get_snapshots_items(git_root)
  if #items == 0 then return end

  local fzf_items = {}
  local hash_map = {}
  for _, item in ipairs(items) do
    table.insert(fzf_items, item.display)
    hash_map[item.display] = item.hash
  end

  local fzf = require("fzf-lua")
  local endur_path = M.config.endur_path

  fzf.fzf_exec(fzf_items, {
    prompt = "Endur Snapshots (" .. vim.fs.basename(git_root) .. ")> ",
    preview = function(selected)
      if not selected or #selected == 0 then return end
      local hash = hash_map[selected[1]]
      if not hash then return end
      return "git -C " .. vim.fn.shellescape(git_root) .. " show --stat -p " .. hash
    end,
    actions = {
      ["default"] = function(selected)
        if not selected or #selected == 0 then return end
        local hash = hash_map[selected[1]]
        if not hash then return end
        vim.ui.input({
          prompt = "Restore ENTIRE repository to snapshot " .. hash:sub(1, 8) .. "? Type 'yes' to confirm: "
        }, function(input)
          if input and input:lower() == "yes" then
            vim.notify("Restoring repository to snapshot " .. hash:sub(1, 8) .. "...")
            local out = vim.fn.system({ endur_path, "restore", hash, git_root })
            if vim.v.shell_error ~= 0 then
              vim.notify("endur: restore failed: " .. out, vim.log.levels.ERROR)
            else
              vim.notify("endur: successfully restored repository to snapshot " .. hash:sub(1, 8), vim.log.levels.INFO)
              vim.cmd("edit!")
            end
          else
            vim.notify("endur: restore aborted", vim.log.levels.INFO)
          end
        end)
      end,
      ["ctrl-f"] = function(selected)
        if not selected or #selected == 0 then return end
        local hash = hash_map[selected[1]]
        if not hash then return end
        vim.ui.input({
          prompt = "Restore current file '" .. current_file .. "' to snapshot " .. hash:sub(1, 8) .. "? (y/N): "
        }, function(input)
          if input and input:lower():match("^y") then
            vim.notify("Restoring " .. current_file .. " to snapshot " .. hash:sub(1, 8) .. "...")
            local out = vim.fn.system({ endur_path, "restore", hash, git_root, "-f", current_file_path })
            if vim.v.shell_error ~= 0 then
              vim.notify("endur: restore failed: " .. out, vim.log.levels.ERROR)
            else
              vim.notify("endur: successfully restored " .. current_file .. " to snapshot " .. hash:sub(1, 8), vim.log.levels.INFO)
              vim.cmd("edit!")
            end
          else
            vim.notify("endur: restore aborted", vim.log.levels.INFO)
          end
        end)
      end
    }
  })
end

-- Open the Endur interactive TUI in a floating terminal
function M.tui()
  local has_snacks, snacks = pcall(require, "snacks")
  if has_snacks and snacks.terminal then
    snacks.terminal.open({ M.config.endur_path, "tui" })
    return
  end

  -- Fallback to standard terminal
  vim.cmd("tabnew")
  vim.fn.termopen({ M.config.endur_path, "tui" })
  vim.cmd("startinsert")
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
  if file_path == "" then return nil, nil end

  local dir = vim.fs.dirname(file_path)
  local git_dir = vim.fs.find(".git", { path = dir, upward = true })[1]
  if not git_dir then return nil, nil end

  local git_root = vim.fs.dirname(git_dir)
  git_root = vim.fn.resolve(git_root)
  
  return M.status_cache[git_root], git_root
end

-- A beautiful statusline component
function M.statusline()
  if not M.config.statusline then return "" end
  local status_info, git_root = M.get_current_repo_status()
  if not status_info or not git_root then return "" end

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

  local repo_name = vim.fs.basename(git_root)
  return string.format("%s %s Endur[%s] %s %%*", hl, icon, repo_name, text)
end

-- Winbar integration (same format as statusline, but can be customized)
function M.winbar()
  return M.statusline()
end

-- A raw statusline component string without highlight codes (useful for lualine, etc.)
function M.statusline_raw()
  local status_info, git_root = M.get_current_repo_status()
  if not status_info or not git_root then return "" end

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

  local repo_name = vim.fs.basename(git_root)
  return string.format("%s Endur[%s] %s", icon, repo_name, text)
end

-- Prints the current repository status in the command line area
function M.print_status()
  local status_info, git_root = M.get_current_repo_status()
  if not status_info or not git_root then
    vim.notify("endur: current file is not in a watched Git repository", vim.log.levels.WARN)
    return
  end

  local status_text = status_info.status
  if status_info.status == "OK" then
    status_text = "Clean"
  elseif status_info.status == "M" then
    status_text = "Modified"
  end

  local repo_name = vim.fs.basename(git_root)
  vim.notify(string.format("endur [%s]: %s (%s)", repo_name, status_text, status_info.details), vim.log.levels.INFO)
end

return M
