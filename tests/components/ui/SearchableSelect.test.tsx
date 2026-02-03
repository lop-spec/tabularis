import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SearchableSelect } from '../../../src/components/ui/SearchableSelect';

describe('SearchableSelect', () => {
  const options = ['Option 1', 'Option 2', 'Another Option'];
  const mockOnChange = vi.fn();

  it('renders with placeholder when no value selected', () => {
    render(
      <SearchableSelect
        value={null}
        options={options}
        onChange={mockOnChange}
        placeholder="Select something"
      />
    );
    expect(screen.getByText('Select something')).toBeInTheDocument();
  });

  it('renders selected value', () => {
    render(
      <SearchableSelect
        value="Option 1"
        options={options}
        onChange={mockOnChange}
      />
    );
    expect(screen.getByText('Option 1')).toBeInTheDocument();
  });

  it('opens dropdown on click', () => {
    render(
      <SearchableSelect
        value={null}
        options={options}
        onChange={mockOnChange}
      />
    );
    
    const button = screen.getByRole('button');
    fireEvent.click(button);
    
    // Check if input (search) is visible, which implies dropdown is open
    expect(screen.getByPlaceholderText('Search...')).toBeInTheDocument();
    expect(screen.getByText('Option 1')).toBeInTheDocument();
  });

  it('filters options based on search input', () => {
    render(
      <SearchableSelect
        value={null}
        options={options}
        onChange={mockOnChange}
      />
    );
    
    fireEvent.click(screen.getByRole('button'));
    
    const searchInput = screen.getByPlaceholderText('Search...');
    fireEvent.change(searchInput, { target: { value: 'Another' } });
    
    expect(screen.getByText('Another Option')).toBeInTheDocument();
    expect(screen.queryByText('Option 1')).not.toBeInTheDocument();
  });

  it('calls onChange when option selected', () => {
    render(
      <SearchableSelect
        value={null}
        options={options}
        onChange={mockOnChange}
      />
    );
    
    fireEvent.click(screen.getByRole('button'));
    fireEvent.click(screen.getByText('Option 2'));
    
    expect(mockOnChange).toHaveBeenCalledWith('Option 2');
  });

  it('displays error state correctly', () => {
    const { container } = render(
      <SearchableSelect
        value="Invalid"
        options={options}
        onChange={mockOnChange}
        hasError={true}
      />
    );
    
    const button = screen.getByRole('button');
    expect(button).toHaveClass('border-red-500');
    expect(screen.getByText('Invalid')).toHaveClass('text-red-400');
  });

  it('is disabled when disabled prop is true', () => {
    render(
      <SearchableSelect
        value={null}
        options={options}
        onChange={mockOnChange}
        disabled={true}
      />
    );
    
    const button = screen.getByRole('button');
    expect(button).toBeDisabled();
    fireEvent.click(button);
    expect(screen.queryByPlaceholderText('Search...')).not.toBeInTheDocument();
  });
});
