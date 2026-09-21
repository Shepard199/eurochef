import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class Ef01Ef03UpdateEvidence extends GhidraScript {
 private void dc(long a,DecompInterface d)throws Exception{Function f=getFunctionContaining(toAddr(a));println(String.format("\n===%08X %s===",a,f==null?"<missing>":f.getName()));if(f!=null){DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted())println(r.getDecompiledFunction().getC());}}
 @Override public void run() throws Exception{DecompInterface d=new DecompInterface();d.openProgram(currentProgram);dc(0x00467500L,d);dc(0x00463620L,d);dc(0x00466F70L,d);d.dispose();}
}