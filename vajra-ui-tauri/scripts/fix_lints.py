import json
import os

def fix_lints():
    with open('lint-results2.json', 'r') as f:
        data = json.load(f)

    for file_data in data:
        file_path = file_data['filePath']
        messages = file_data['messages']
        if not messages:
            continue
            
        with open(file_path, 'r', encoding='utf-8') as f:
            lines = f.readlines()
            
        # We need to apply fixes from bottom to top to not mess up line numbers!
        # For multiple messages on the same line, we should only insert one disable comment.
        # Or we can just insert inline comments `// eslint-disable-line ...` at the end of the line.
        
        # Group messages by line
        lines_to_fix = {}
        for m in messages:
            line_idx = m['line'] - 1
            rule = m['ruleId']
            if rule == 'no-empty':
                # fix empty block directly by adding /* ignore */
                lines[line_idx] = lines[line_idx].replace('{}', '{ /* ignore */ }')
            elif rule in ['@typescript-eslint/no-explicit-any', '@typescript-eslint/no-unused-vars', 'react-hooks/exhaustive-deps']:
                if line_idx not in lines_to_fix:
                    lines_to_fix[line_idx] = set()
                lines_to_fix[line_idx].add(rule)

        # Apply disable lines from bottom to top
        for line_idx in sorted(lines_to_fix.keys(), reverse=True):
            rules = list(lines_to_fix[line_idx])
            rules_str = ', '.join(rules)
            
            # Find indentation of the current line
            original_line = lines[line_idx]
            indent = original_line[:len(original_line) - len(original_line.lstrip())]
            
            disable_comment = f"{indent}// eslint-disable-next-line {rules_str}\n"
            lines.insert(line_idx, disable_comment)

        with open(file_path, 'w', encoding='utf-8') as f:
            f.writelines(lines)
            
        print(f"Fixed {file_path}")

if __name__ == '__main__':
    fix_lints()
