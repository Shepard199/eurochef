import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class StandardMonsterFirstUpdateFiveEvidence extends GhidraScript {
 private void dc(long a,String name,DecompInterface d)throws Exception{Function f=getFunctionContaining(toAddr(a));println(String.format("\n===%s %08X %s===",name,a,f==null?"<missing>":f.getName()));if(f!=null){DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted())println(r.getDecompiledFunction().getC());}}
 @Override public void run() throws Exception{DecompInterface d=new DecompInterface();d.openProgram(currentProgram);dc(0x00463230L,"EW09",d);dc(0x00463970L,"EF01",d);dc(0x00467540L,"EF03",d);dc(0x00462BE0L,"LARGE",d);d.dispose();}
}