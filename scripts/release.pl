#!/usr/bin/env perl

use strict;
use warnings;
use IPC::Open3;
use Symbol 'gensym';
use Term::ANSIColor;

# Read a file.
sub read_file {
    my ($filename) = @_;
    open(my $fh, '<', $filename) or die "Cannot open $filename: $!\n";
    local $/;
    my $content = <$fh>;
    close($fh);
    return $content;
}

# Write to a file.
sub write_file {
    my ($filename, $content) = @_;
    open(my $fh, '>', $filename) or die "Cannot write $filename: $!\n";
    print $fh $content;
    close($fh);
}

# Print the step message.
my $current_step = 1;
sub print_step_message {
    my ($message) = @_;
    print color('cyan') . "\n[$current_step/7] $message\n" . color('reset');
    $current_step++;
}

# Run a command or die with an error message.
sub run_command {
    my ($command, $error_message) = @_;
    my $command_str = join(' ', map { "\"$_\"" } @$command);
    print color('yellow') . "      Running command: $command_str\n" . color('reset');

    # Capture output using IPC::Open3.
    my $pid = open3(my $chld_in, my $chld_out, my $chld_err = gensym(), @$command);
    close($chld_in);

    # Read and print stdout with 6-space indentation
    while (defined(my $line = <$chld_out>)) {
        print "      stdout | $line";
    }
    close($chld_out);

    # Read and print stderr with 6-space indentation
    while (defined(my $line = <$chld_err>)) {
        print "      stderr | $line";
    }
    close($chld_err);

    # Wait for the command to complete and check the exit code.
    waitpid($pid, 0);
    my $exit_code = $? >> 8;
    die color('red') . "      Error: $error_message\n" . color('reset') if $exit_code != 0;
}

# Parse command line argument(s).
my $version = shift @ARGV;
die "Usage: $0 <version>\n" unless defined $version;

# Handle the version.
$version =~ s/^v//;
unless ($version =~ /^\d+\.\d+\.\d+/) {
    die color('red') . "Error: Invalid version format. Expected format: X.Y.Z (e.g., 0.1.13)\n" . color('reset');
}

print color('green') . "Releasing version: $version\n" . color('reset');

# Update Cargo.toml.
my $toml_filename = 'Cargo.toml';
print_step_message("Updating $toml_filename...");
my $toml = read_file($toml_filename);
$toml =~ s/^version = ".*"/version = "$version"/m;
write_file($toml_filename, $toml);
print color('green') . "      Updated version to $version\n" . color('reset');

# Update Cargo.lock.
print_step_message("Updating Cargo.lock...");
run_command(
    [qw(cargo check --workspace)],
    "Failed to update Cargo.lock"
);
print color('green') . "      Cargo.lock updated\n" . color('reset');

# Git commit and tag.
print_step_message("Creating git commit and tag...");
print color('yellow') . "      Do you want to commit changes and create tag v$version? (y/N): " . color('reset');
my $confirm = <STDIN>;
chomp $confirm;
if ($confirm =~ /^[yY]/) {
    run_command(
        [qw(git add .)],
        "Failed to stage files"
    );
    run_command(
        ["git", "commit", "-m", "chore: Release version $version."],
        "Failed to commit"
    );
    run_command(
        ["git", "tag", "v$version"],
        "Failed to create tag"
    );
    print color('green') . "      Created commit and tag v$version\n" . color('reset');
} else {
    print color('yellow') . "      Skipped git commit and tag\n" . color('reset');
}

# Publish fracindex.
my $name = "fracindex";
print_step_message("Publishing $name...");
print color('yellow') . "      Do you want to publish $name? (y/N): " . color('reset');
$confirm = <STDIN>;
chomp $confirm;
if ($confirm =~ /^[yY]/) {
    run_command(
        [qw(cargo publish --package fracindex)],
        "Failed to publish $name"
    );
    print color('green') . "      Published $name\n" . color('reset');
} else {
    print color('yellow') . "      Skipped publishing $name\n" . color('reset');
}

# Git push.
print_step_message("Pushing to remote...");
print color('yellow') . "      Do you want to push commits and tags to remote? (y/N): " . color('reset');
$confirm = <STDIN>;
chomp $confirm;
if ($confirm =~ /^[yY]/) {
    run_command(
        ["git", "push", "origin", "v$version"],
        "Failed to push tag"
    );
    run_command([qw(git push origin HEAD)], "Failed to push commits");
    print color('green') . "      Pushed commits and tags\n" . color('reset');
} else {
    print color('yellow') . "      Skipped git push\n" . color('reset');
}

print color('green') . "\n      Release completed successfully!\n" . color('reset');
