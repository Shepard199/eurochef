import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class MinionCtorEvidence extends GhidraScript {
 private void dc(long a,DecompInterface d)throws Exception{Function f=getFunctionContaining(toAddr(a));println(String.format("\n===%08X===",a));if(f!=null){DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted())println(r.getDecompiledFunction().getC());}}
 @Override public void run() throws Exception {DecompInterface d=new DecompInterface();d.openProgram(currentProgram);dc(0x004641D0L,d);dc(0x00463BD0L,d);dc(0x004514F0L,d);d.dispose();}
}