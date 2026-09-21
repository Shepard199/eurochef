import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.Reference;
import java.util.LinkedHashSet;
import java.util.Set;

public class CurrentAttackerRawRefsEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  long[] globals={0x007B2A10L,0x007B2A14L};
  DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
  Set<Function> fs=new LinkedHashSet<>();
  InstructionIterator it=currentProgram.getListing().getInstructions(true);
  while(it.hasNext()){
   Instruction ins=it.next();
   for(int op=0;op<ins.getNumOperands();op++){
    for(Object obj:ins.getOpObjects(op)){
     if(obj instanceof Scalar){
      long v=((Scalar)obj).getUnsignedValue();
      for(long g:globals) if(v==g){
       Function f=getFunctionContaining(ins.getAddress());
       println(String.format("IMM 0x%08X @ %s %s %s",g,ins.getAddress(),ins.toString(),f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
       if(f!=null) fs.add(f);
      }
     }
     if(obj instanceof Address){
      long v=((Address)obj).getOffset();
      for(long g:globals) if(v==g){
       Function f=getFunctionContaining(ins.getAddress());
       println(String.format("ADDR 0x%08X @ %s %s %s",g,ins.getAddress(),ins.toString(),f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
       if(f!=null) fs.add(f);
      }
     }
    }
   }
  }
  for(Function f:fs){
   println("\n=== "+f.getName()+" "+f.getEntryPoint()+" ===");
   DecompileResults r=d.decompileFunction(f,90,monitor);
   if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
  }
  d.dispose();
 }
}