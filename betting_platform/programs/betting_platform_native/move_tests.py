#!/usr/bin/env python3
"""
Script to move test modules from src/ to tests/ directory
"""
import os
import re
import shutil
from pathlib import Path

def find_test_modules(src_dir):
    """Find all files containing #[cfg(test)] modules"""
    test_files = []
    for root, dirs, files in os.walk(src_dir):
        for file in files:
            if file.endswith('.rs'):
                file_path = os.path.join(root, file)
                with open(file_path, 'r') as f:
                    content = f.read()
                    if '#[cfg(test)]' in content and 'mod tests' in content:
                        test_files.append(file_path)
    return test_files

def extract_test_module(file_path):
    """Extract test module from a source file"""
    with open(file_path, 'r') as f:
        lines = f.readlines()
    
    # Find the start of test module
    test_start = -1
    for i, line in enumerate(lines):
        if '#[cfg(test)]' in line:
            # Check if next line or line after contains 'mod tests'
            if i + 1 < len(lines) and 'mod tests' in lines[i + 1]:
                test_start = i
                break
            elif i + 2 < len(lines) and 'mod tests' in lines[i + 2]:
                test_start = i
                break
    
    if test_start == -1:
        return None, lines
    
    # Find the end of test module by counting braces
    brace_count = 0
    test_end = test_start
    in_test_module = False
    
    for i in range(test_start, len(lines)):
        line = lines[i]
        if 'mod tests' in line and '{' in line:
            in_test_module = True
            brace_count = 1
            test_end = i
        elif in_test_module:
            brace_count += line.count('{') - line.count('}')
            if brace_count == 0:
                test_end = i
                break
    
    # Extract test module
    test_module = lines[test_start:test_end + 1]
    
    # Remove test module from source
    remaining_lines = lines[:test_start] + lines[test_end + 1:]
    
    # Clean up extra blank lines at the end
    while remaining_lines and remaining_lines[-1].strip() == '':
        remaining_lines.pop()
    
    return test_module, remaining_lines

def get_imports_from_file(file_path):
    """Extract necessary imports from the source file"""
    with open(file_path, 'r') as f:
        lines = f.readlines()
    
    imports = []
    for line in lines:
        if line.startswith('use ') and 'test' not in line.lower():
            imports.append(line)
    
    return imports

def create_test_file(src_file_path, test_module, test_dir):
    """Create a test file in the tests directory"""
    # Calculate relative module path
    src_dir = Path('src')
    rel_path = Path(src_file_path).relative_to(src_dir)
    
    # Create test file path
    test_file_name = rel_path.stem + '_tests.rs'
    test_subdir = rel_path.parent
    test_file_path = test_dir / test_subdir / test_file_name
    
    # Create directory if it doesn't exist
    test_file_path.parent.mkdir(parents=True, exist_ok=True)
    
    # Calculate module imports
    module_parts = []
    for part in rel_path.parts[:-1]:
        if part != 'mod.rs':
            module_parts.append(part)
    if rel_path.stem != 'mod':
        module_parts.append(rel_path.stem)
    
    module_path = '::'.join(module_parts)
    
    # Build test file content
    content = [
        f'use betting_platform_native::{module_path}::*;\n',
        'use solana_program::pubkey::Pubkey;\n',
        'use solana_program::clock::Clock;\n',
        'use solana_program::program_error::ProgramError;\n',
        '\n'
    ]
    
    # Extract test functions from module
    test_functions = []
    in_test_fn = False
    current_fn = []
    indent_level = 0
    
    for line in test_module:
        if '#[test]' in line:
            in_test_fn = True
            current_fn = [line]
        elif in_test_fn:
            current_fn.append(line)
            if '{' in line:
                indent_level += line.count('{') - line.count('}')
            elif '}' in line:
                indent_level += line.count('{') - line.count('}')
                if indent_level == 0 and 'fn ' in ''.join(current_fn):
                    # End of test function
                    test_functions.append(current_fn)
                    in_test_fn = False
                    current_fn = []
    
    # Add test functions without the mod wrapper
    for test_fn in test_functions:
        # Remove extra indentation from being inside mod tests
        cleaned_fn = []
        for line in test_fn:
            if line.strip():
                # Remove one level of indentation
                if line.startswith('    '):
                    cleaned_fn.append(line[4:])
                else:
                    cleaned_fn.append(line)
            else:
                cleaned_fn.append(line)
        content.extend(cleaned_fn)
        content.append('\n')
    
    # Write test file
    with open(test_file_path, 'w') as f:
        f.writelines(content)
    
    return test_file_path

def main():
    src_dir = Path('src')
    test_dir = Path('tests')
    
    # Find all test files
    print("Finding test modules...")
    test_files = find_test_modules(src_dir)
    print(f"Found {len(test_files)} files with test modules")
    
    # Process each file
    processed = 0
    for file_path in test_files:
        print(f"\nProcessing {file_path}...")
        
        # Extract test module
        test_module, remaining_lines = extract_test_module(file_path)
        
        if test_module:
            # Create test file
            test_file_path = create_test_file(file_path, test_module, test_dir)
            print(f"  Created test file: {test_file_path}")
            
            # Update source file
            with open(file_path, 'w') as f:
                f.writelines(remaining_lines)
            print(f"  Removed test module from source file")
            
            processed += 1
        else:
            print(f"  No test module found")
    
    print(f"\nProcessed {processed} files")

if __name__ == '__main__':
    main()